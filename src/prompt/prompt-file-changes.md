# UDIFFX File Changes Instructions

When modifying files, output exactly one `UDIFFX_FILE_CHANGES` bracket tag block. This is the only supported mechanism for creating, updating, appending to, copying, renaming, moving, or deleting files.

Do not use tools or any other file-editing format.

```text
[[[UDIFFX_FILE_CHANGES]]]
_file directives_
[[[/UDIFFX_FILE_CHANGES]]]
```

Do not include explanations, markdown, comments, or other content outside the block.

Every tag must:

- occupy its own line
- begin at column 1
- begin with `[[[` and end with `]]]`
- contain only the bracket tag on that line

## File Directives

### FILE_NEW - Create a File 

```text
[[[FILE_NEW file_path="path/to/file"]]]
_complete file content_
[[[/FILE_NEW]]]
```

Use `FILE_NEW` only when creating a new file. Include the complete file content.

### FILE_PATCH - Modify an Existing File

This is a specialized patch format, with the markers defined below (we do not have `***` markers with this patch format)

```text
[[[FILE_PATCH file_path="path/to/file"]]]
@@
-old content
+new content

@@
 context line
-old value
+new value
[[[/FILE_PATCH]]] <<<--- IMPORTANT: Always end FILE_PATCH with this end backet tag
```

Use `FILE_PATCH` only when existing content must change.

For each file:

- Emit at most one `FILE_PATCH`.
- Put all in-place changes in that directive using multiple `@@` hunks.
- Keep patches minimal.
- Do not use line-numbered hunk headers.
- Do not include `---` or `+++` file headers.
- Do not use `FILE_PATCH` for content added only at end-of-file.

Each hunk begins with:

```
@@
```

Every hunk body line must begin with exactly one prefix:

| Prefix | Meaning                                                            |
| ------ | ------------------------------------------------------------------ |
| ` `    | Unchanged context, surround to ground following or previous change |
| `-`    | Removed line                                                       |
| `+`    | Added line                                                         |
| `~`    | Omitted middle portion of a continuous removed range               |

Context and removal lines must exactly match the original file, including indentation and whitespace.

When replacing content, include the removed lines. Do not provide only the replacement.

Important, when doing `-` for more than 6 lines, use the `~` when removing a large continuous block

Like this:

```
[[[FILE_PATCH file_path="path/to/file"]]]
@@
-first to remove line
-second to remove line
~
-second-to-last to remove line
-last to remove line
+replacement
[[[/FILE_PATCH]]]
```

- `~` may appear only between removal lines.
- `~` represents one continuous removed region.
- No context or addition lines may appear inside the omitted region.
- Prefer it when removing more than four or five consecutive lines.

### FILE_APPEND - Append to the End of a File

```text
[[[FILE_APPEND file_path="path/to/file"]]]
_content to append_
[[[/FILE_APPEND]]] <<<--- IMPORTANT: Always end FILE_APPEND with this end backet tag
```

Use `FILE_APPEND` when content is added only at the end of a file.

Typical uses include adding functions, entries, list items, or sections at end-of-file.

Rules:

- Do not represent a pure append using a `FILE_PATCH` containing only `+` lines.
- If the file does not exist, `FILE_APPEND` creates it.
- A file may have one `FILE_PATCH` and one `FILE_APPEND` when both existing content changes and end-of-file additions are required.

Decision rule:

- Create a new complete file → `FILE_NEW`
- Change existing content → `FILE_PATCH`
- Add content only at end-of-file → `FILE_APPEND`
- Change existing content and append content → one `FILE_PATCH` plus one `FILE_APPEND`

### FILE_COPY - Copy a File

```text
[[[FILE_COPY from_path="source" to_path="destination" /]]]
```

### FILE_RENAME - Rename or Move a File

```text
[[[FILE_RENAME from_path="source" to_path="destination" /]]]
```

### FILE_DELETE - Delete a File

```text
[[[FILE_DELETE file_path="path/to/file" /]]]
```

`FILE_COPY`, `FILE_RENAME`, and `FILE_DELETE` are self-closing and must end with ` /]]]`.

## General Rules

- Every file directive must be inside exactly one `UDIFFX_FILE_CHANGES` block.
- No file directive may appear outside that block.
- `UDIFFX_FILE_CHANGES`, `FILE_NEW`, `FILE_PATCH`, and `FILE_APPEND` must have matching closing tags.
- Block closing tags must use the form `[[[/TAG_NAME]]]`.
- Directives must not be nested inside other directives.
- The `file_path` attribute is the sole source of truth for the target file.
- Do not invent files or paths.
- Preserve exact formatting, indentation, and whitespace.
- Preserve existing comments verbatim unless explicitly asked to change them.
- Do not add trivial explanatory comments.
- Patch context and anchors must exactly match the original file.
- Code-fence language identifiers are only syntax highlighting and are not part of the file content.
- Think through all file changes before emitting the block.
- The response must end immediately after `[[[/UDIFFX_FILE_CHANGES]]]`.

## Complete Example

```text
[[[UDIFFX_FILE_CHANGES]]]
[[[FILE_NEW file_path="src/hello.rs"]]]
pub fn hello() {
    println!("Hello from hello.rs");
}
[[[/FILE_NEW]]]

[[[FILE_PATCH file_path="src/main.rs"]]]
@@
+mod hello;

 fn main() {
-    println!("Old message");
+    hello::hello();
 }

@@
 fn helper() {
-old_logic();
+new_logic();
 }
[[[/FILE_PATCH]]] <<<--- IMPORTANT: Always end FILE_PATCH with this end backet tag

[[[FILE_APPEND file_path="CHANGELOG.md"]]]

## Added

- Hello module
[[[/FILE_APPEND]]]

[[[FILE_COPY from_path="docs/OLD_README.md" to_path="docs/README.backup.md" /]]]

[[[FILE_RENAME from_path="docs/OLD_README.md" to_path="README.md" /]]]

[[[FILE_DELETE file_path="temp_notes.txt" /]]]
[[[/UDIFFX_FILE_CHANGES]]]
```

Before returning, verify that:

- there is exactly one `[[[UDIFFX_FILE_CHANGES]]]` tag
- there is exactly one `[[[/UDIFFX_FILE_CHANGES]]]` tag
- every block directive has its matching closing tag
- every self-closing directive ends with ` /]]]`
- no directive is nested inside another directive
- each file has at most one `FILE_PATCH`
- pure end-of-file additions use `FILE_APPEND`
- every patch hunk line has a valid prefix
- the last file directive is fully closed
- the final non-whitespace line is exactly:

```text
[[[/UDIFFX_FILE_CHANGES]]]
```

## Exact Closing-Tag Syntax

Block directives use slash-style closing tags.

Correct:

[[[/UDIFFX_FILE_CHANGES]]]
[[[/FILE_NEW]]]
[[[/FILE_PATCH]]]
[[[/FILE_APPEND]]]
