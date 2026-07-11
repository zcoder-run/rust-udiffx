use crate::{Content, Error, FileChanges, FileDirective, Result};
use markex::tag::{self, TagFence};

/// A triple-square-bracket fence for clearly separating structured payloads.
pub const FENCE_UDIFFX: TagFence = TagFence {
	name: "udiffx",
	open_delim: "[[[",
	close_delim: "]]]",
	close_delim_alts: Some(&["]]"]),
	closing_tag_prefix: "/",
	self_closing_suffix: "/",
};

/// Extracts the first `FILE_CHANGES` block from the input string.
pub fn extract_file_changes(input: &str, extrude_other_content: bool) -> Result<(FileChanges, Option<String>)> {
	let parts = tag::extract_with_fence(input, &["UDIFFX_FILE_CHANGES"], extrude_other_content, FENCE_UDIFFX);
	let (tag_elems, extruded) = if extrude_other_content {
		let (elems, s) = parts.into_with_extrude_content();
		(elems, Some(s))
	} else {
		(parts.into_tag_elems(), None)
	};

	let Some(changes_tag) = tag_elems.into_iter().next() else {
		return Ok((FileChanges::new(Vec::new()), extruded));
	};

	let inner_content = changes_tag.content;

	// -- Pre-process to expand potential self-closing tags (since markex might skip them)
	let child_parts = tag::extract_with_fence(
		&inner_content,
		&[
			"FILE_NEW",
			"FILE_PATCH",
			"FILE_APPEND",
			"FILE_COPY",
			"FILE_RENAME",
			"FILE_DELETE",
		],
		false,
		FENCE_UDIFFX,
	);

	let mut directives = Vec::new();

	for elem in child_parts.into_tag_elems() {
		let tag_name = elem.tag.clone();
		let mut attrs = elem.attrs.unwrap_or_default();

		// Try to find a path for better reporting if it fails.
		let file_path_attr = attrs
			.get("file_path")
			.or_else(|| attrs.get("to_path"))
			.or_else(|| attrs.get("from_path"))
			.cloned();

		let directive_res = (|| -> Result<FileDirective> {
			match tag_name.as_str() {
				"FILE_NEW" => {
					let file_path = attrs
						.remove("file_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_NEW", "file_path"))?;

					Ok(FileDirective::New {
						file_path,
						content: Content::from_raw(elem.content),
					})
				}
				"FILE_PATCH" => {
					let file_path = attrs
						.remove("file_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_PATCH", "file_path"))?;

					Ok(FileDirective::Patch {
						file_path,
						content: Content::from_raw(elem.content),
					})
				}
				"FILE_APPEND" => {
					let file_path = attrs
						.remove("file_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_APPEND", "file_path"))?;

					Ok(FileDirective::Append {
						file_path,
						content: Content::from_raw(elem.content),
					})
				}
				"FILE_COPY" => {
					let from_path = attrs
						.remove("from_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_COPY", "from_path"))?;
					let to_path = attrs
						.remove("to_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_COPY", "to_path"))?;

					Ok(FileDirective::Copy { from_path, to_path })
				}
				"FILE_RENAME" => {
					let from_path = attrs
						.remove("from_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_RENAME", "from_path"))?;
					let to_path = attrs
						.remove("to_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_RENAME", "to_path"))?;

					Ok(FileDirective::Rename { from_path, to_path })
				}
				"FILE_DELETE" => {
					let file_path = attrs
						.remove("file_path")
						.ok_or_else(|| Error::parse_missing_attribute("FILE_DELETE", "file_path"))?;

					Ok(FileDirective::Delete { file_path })
				}
				_ => Err(Error::parse_unknown_directive_tag(tag_name.to_string())),
			}
		})();

		let directive = match directive_res {
			Ok(d) => d,
			Err(err) => FileDirective::Fail {
				kind: tag_name,
				file_path: file_path_attr,
				error_msg: err.to_string(),
			},
		};

		directives.push(directive);
	}

	Ok((FileChanges::new(directives), extruded))
}
