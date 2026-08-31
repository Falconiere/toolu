PY_MAX_FILE=$(python_max_file_lines)
LINE_COUNT=$(count_python_code_lines "$FILE_PATH")
if [[ "$LINE_COUNT" -gt "$PY_MAX_FILE" ]]; then
  add_error "File exceeds ${PY_MAX_FILE}-line limit: $FILE_PATH ($LINE_COUNT code lines, blanks/comments excluded) — split into submodules. Override via lang.python.maxFileLines."
fi

