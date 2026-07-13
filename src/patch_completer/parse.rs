use super::TILDE_MIN_ANCHOR_LINES;
use super::types::TildeRange;
use crate::{Error, Result, U_HUNK_DELIM};
use std::borrow::Cow;

// region:    --- Public Helpers

/// Returns `true` if the raw patch text contains at least one actionable hunk
/// (i.e., a hunk with at least one `+` or `-` line).
pub fn has_actionable_hunks(patch_raw: &str) -> bool {
	let patch_raw: Cow<'_, str> = if patch_raw.contains("\r\n") {
		Cow::Owned(patch_raw.replace("\r\n", "\n"))
	} else {
		Cow::Borrowed(patch_raw)
	};

	let raw_hunks = collect_raw_hunks(&patch_raw);
	if !raw_hunks.is_empty() {
		return true;
	}

	// Retry with sanitized content
	let sanitized = sanitize_wrapper_meta_lines(&patch_raw);
	let raw_hunks = collect_raw_hunks(&sanitized);
	!raw_hunks.is_empty()
}

/// Splits a raw simplified patch (numberless `@@` hunks) into individual hunk strings.
///
/// Each returned `String` contains a single `@@` header followed by its body lines.
/// The splitting reuses the same parsing logic as `complete`: CRLF normalization,
/// sanitize wrapper meta lines, trailing whitespace stripping, and the actionable
/// check (only hunks with at least one `+` or `-` line are included).
pub fn split_raw_hunks(patch_raw: &str) -> Vec<String> {
	let patch_raw: Cow<'_, str> = if patch_raw.contains("\r\n") {
		Cow::Owned(patch_raw.replace("\r\n", "\n"))
	} else {
		Cow::Borrowed(patch_raw)
	};

	let raw_hunks = collect_raw_hunks(&patch_raw);

	if !raw_hunks.is_empty() {
		// Reconstruct each hunk as a self-contained patch string with its @@ header
		return raw_hunks
			.into_iter()
			.map(|lines| {
				let mut hunk_str = format!("{}\n", U_HUNK_DELIM);
				for line in lines {
					hunk_str.push_str(line);
					hunk_str.push('\n');
				}
				hunk_str
			})
			.collect();
	}

	// If strict parse produced no actionable hunks, retry with sanitized content
	let sanitized = sanitize_wrapper_meta_lines(&patch_raw);
	let raw_hunks = collect_raw_hunks(&sanitized);

	// Reconstruct each hunk as a self-contained patch string with its @@ header
	raw_hunks
		.into_iter()
		.map(|lines| {
			let mut hunk_str = format!("{}\n", U_HUNK_DELIM);
			for line in lines {
				hunk_str.push_str(line);
				hunk_str.push('\n');
			}
			hunk_str
		})
		.collect()
}

/// Checks whether a raw hunk contains any `~` (tilde range-remove) markers.
pub fn has_tilde_ranges(hunk_raw: &str) -> bool {
	hunk_raw.lines().any(|l| l.trim() == "~")
}

// endregion: --- Public Helpers

// region:    --- Internal Parsing

/// Collects raw hunk bodies from patch text, returning each hunk as a `Vec<&str>` of body lines.
///
/// Shared by both `split_raw_hunks` and `complete` to avoid duplicating the parsing logic.
pub(super) fn collect_raw_hunks(patch_text: &str) -> Vec<Vec<&str>> {
	let patch_text = remove_final_malformed_wrapper_terminator(patch_text);
	let mut raw_hunks: Vec<Vec<&str>> = Vec::new();
	let mut lines = patch_text.lines().peekable();

	while let Some(line) = lines.next() {
		let trimmed = line.trim();

		if trimmed.starts_with(U_HUNK_DELIM) {
			let mut hunk_lines = Vec::new();
			while let Some(next_line) = lines.peek() {
				let next_trimmed = next_line.trim();
				if next_trimmed.starts_with("@@") {
					break;
				}
				hunk_lines.push(lines.next().unwrap());
			}

			// Strip trailing empty lines that lack a valid diff prefix.
			// These are artefacts of the raw patch text (e.g. a trailing newline)
			// and would otherwise be mis-counted as context lines.
			while hunk_lines.last().is_some_and(|l| l.trim().is_empty()) {
				hunk_lines.pop();
			}

			let has_add = hunk_lines.iter().any(|l| l.starts_with('+'));
			let has_remove = hunk_lines.iter().any(|l| l.starts_with('-'));
			let has_tilde = hunk_lines.iter().any(|l| l.trim() == "~");
			let is_actionable = has_add || has_remove || has_tilde;

			if is_actionable {
				raw_hunks.push(hunk_lines);
			}
		}
	}

	raw_hunks
}

/// Collects raw hunk bodies from patch text while ignoring recognized wrapper meta lines.
///
/// This is used as a resilience path when wrapper lines like `*** End Patch` appear
/// inside or after otherwise valid hunks. Unknown `*** ...` lines are preserved.
pub(super) fn collect_raw_hunks_sanitized(patch_text: &str) -> Vec<Vec<&str>> {
	let patch_text = remove_final_malformed_wrapper_terminator(patch_text);
	let mut raw_hunks: Vec<Vec<&str>> = Vec::new();
	let mut lines = patch_text.lines().peekable();

	while let Some(line) = lines.next() {
		let trimmed = line.trim();

		if is_wrapper_meta_line(trimmed) {
			continue;
		}

		if trimmed.starts_with(U_HUNK_DELIM) {
			let mut hunk_lines = Vec::new();
			while let Some(next_line) = lines.peek() {
				let next_trimmed = next_line.trim();
				if next_trimmed.starts_with(U_HUNK_DELIM) {
					break;
				}
				let next_line = lines.next().unwrap();
				if is_wrapper_meta_line(next_trimmed) {
					continue;
				}
				hunk_lines.push(next_line);
			}

			// Strip trailing empty lines that lack a valid diff prefix.
			// These are artefacts of the raw patch text (e.g. a trailing newline)
			// and would otherwise be mis-counted as context lines.
			while hunk_lines.last().is_some_and(|l| l.trim().is_empty()) {
				hunk_lines.pop();
			}

			let has_add = hunk_lines.iter().any(|l| l.starts_with('+'));
			let has_remove = hunk_lines.iter().any(|l| l.starts_with('-'));
			let has_tilde = hunk_lines.iter().any(|l| l.trim() == "~");
			let is_actionable = has_add || has_remove || has_tilde;

			if is_actionable {
				raw_hunks.push(hunk_lines);
			}
		}
	}

	raw_hunks
}

/// Validates `~` markers in a hunk and returns parsed `TildeRange` entries.
/// Returns `Err` if any `~` is not properly bracketed by at least `TILDE_MIN_ANCHOR_LINES`
/// removal lines on each side, or if `~` appears between non-removal lines.
pub(super) fn validate_and_parse_tilde_ranges(hunk_lines: &[&str]) -> Result<Vec<TildeRange>> {
	let tilde_indices: Vec<usize> = hunk_lines
		.iter()
		.enumerate()
		.filter(|(_, l)| l.trim() == "~")
		.map(|(i, _)| i)
		.collect();

	if tilde_indices.is_empty() {
		return Ok(Vec::new());
	}

	let mut ranges = Vec::new();

	for &tilde_idx in &tilde_indices {
		// Collect consecutive `-` lines immediately above the `~`
		let mut top_anchors: Vec<usize> = Vec::new();
		let mut j = tilde_idx;
		while j > 0 {
			j -= 1;
			if hunk_lines[j].starts_with('-') {
				top_anchors.push(j);
			} else {
				break;
			}
		}
		top_anchors.reverse();

		// Collect consecutive `-` lines immediately below the `~`
		let mut bottom_anchors: Vec<usize> = Vec::new();
		let mut k = tilde_idx + 1;
		while k < hunk_lines.len() {
			if hunk_lines[k].starts_with('-') {
				bottom_anchors.push(k);
				k += 1;
			} else {
				break;
			}
		}

		// Validate minimum anchor counts
		if top_anchors.len() < TILDE_MIN_ANCHOR_LINES {
			return Err(Error::patch_completion(format!(
				"Tilde range-remove `~` at hunk line {} requires at least {} removal lines above, found {}",
				tilde_idx + 1,
				TILDE_MIN_ANCHOR_LINES,
				top_anchors.len()
			)));
		}
		if bottom_anchors.len() < TILDE_MIN_ANCHOR_LINES {
			return Err(Error::patch_completion(format!(
				"Tilde range-remove `~` at hunk line {} requires at least {} removal lines below, found {}",
				tilde_idx + 1,
				TILDE_MIN_ANCHOR_LINES,
				bottom_anchors.len()
			)));
		}

		ranges.push(TildeRange {
			top_anchor_hl_indices: top_anchors,
			tilde_hl_index: tilde_idx,
			bottom_anchor_hl_indices: bottom_anchors,
		});
	}

	Ok(ranges)
}

// endregion: --- Internal Parsing

pub(super) fn is_wrapper_meta_line(trimmed: &str) -> bool {
	trimmed == "*** Begin Patch" || trimmed == "*** End Patch" || trimmed.starts_with("*** Update File:")
}

pub(super) fn sanitize_wrapper_meta_lines(patch_raw: &str) -> String {
	let patch_raw = remove_final_malformed_wrapper_terminator(patch_raw);
	let mut out = String::new();
	for line in patch_raw.lines() {
		if is_wrapper_meta_line(line.trim()) {
			continue;
		}
		out.push_str(line);
		out.push('\n');
	}
	out
}

fn remove_final_malformed_wrapper_terminator(patch_raw: &str) -> &str {
	let mut offset = 0;
	let mut final_non_empty_line: Option<(usize, &str)> = None;

	for segment in patch_raw.split_inclusive('\n') {
		let line = segment.strip_suffix('\n').unwrap_or(segment);
		if !line.trim().is_empty() {
			final_non_empty_line = Some((offset, line));
		}
		offset += segment.len();
	}

	let Some((line_start, line)) = final_non_empty_line else {
		return patch_raw;
	};

	if is_malformed_wrapper_terminator(line) {
		&patch_raw[..line_start]
	} else {
		patch_raw
	}
}

fn is_malformed_wrapper_terminator(line: &str) -> bool {
	if line.starts_with([' ', '+', '-']) {
		return false;
	}

	let Some(marker_content) = line.trim().strip_prefix("***") else {
		return false;
	};

	let mut expected_chars = "endpatch".chars();
	for character in marker_content.chars() {
		if character.is_ascii_alphabetic() {
			if expected_chars.next() != Some(character.to_ascii_lowercase()) {
				return false;
			}
		} else if !character.is_ascii_whitespace() && !character.is_ascii_punctuation() {
			return false;
		}
	}

	expected_chars.next().is_none()
}
