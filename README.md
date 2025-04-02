# inspect-grep

Fast command-line tool for messages in collections of Inspect .eval logs.
Uses concurrent direct zip reading to process only the files that match the search criteria, in parallel.

## Features

- Search through .eval files (zipped JSON archives) with blazing fast performance
- Filter messages by:
  - Sample IDs (using regex patterns)
  - Epoch numbers (single, multiple, or ranges)
  - Message roles (system, user, assistant, tool)
  - Message content (using regex patterns)
- Parallel processing for improved performance
- Colored output with syntax highlighting
- Support for both single files and directories

## Installation

```bash
cargo install inspect-grep
```

## Usage

Basic syntax:
```bash
inspect-grep <path> [options]
```

### Arguments

- `path`: Path to a .eval file or directory containing .eval files (required)

### Options

- `-m, --message-regex <pattern>`: Search for messages matching the regex pattern
- `-s, --samples <pattern>`: Filter by sample ID using regex pattern
- `-e, --epochs <filter>`: Filter by epoch number (default: "all")
  - Format: "all", "1,2,3", or "1-5"
- `-r, --roles <roles>`: Filter by message roles (comma-separated)
  - Available roles: system, user, assistant, tool

### Examples

Search all messages in a single file:
```bash
inspect-grep path/to/file.eval
```

Search for specific content across all .eval files in a directory:
```bash
inspect-grep path/to/directory -m "error|warning"
```

Filter by sample ID and epoch:
```bash
inspect-grep path/to/file.eval -s "sample_123" -e "1-5"
```

Filter by message roles:
```bash
inspect-grep path/to/file.eval -r "system,assistant"
```

## Output Format

The tool displays messages in a clear, color-coded format:
- File name: Cyan
- Sample ID: Yellow
- Epoch number: Green
- Message roles:
  - System: Magenta
  - User: Blue
  - Assistant: Green
  - Tool: Yellow
- Matching content (when using --message-regex): Red and bold

## License

MIT License 