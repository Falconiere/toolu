# lint-syntax.sh — remove string contents before suppression-policy matching.
#
# These scanners are deliberately lexical rather than regex-only. Directive
# text is meaningful in a real JavaScript comment and Rust attributes are
# meaningful in Rust code; the same bytes inside a string are documentation.

gr_ls_script_syntax_reset() {
  GR_LS_JS_STATE=code
  GR_LS_JS_BRACE_DEPTH=0
  GR_LS_JS_BRACE_STACK=''
  GR_LS_JS_JSX_DEPTH=0
  GR_LS_JS_JSX_STACK=''
  GR_LS_JS_TAG_KIND=''
  GR_LS_JS_TAG_LAST=''
  GR_LS_JS_REGEX_CLASS=0
  GR_LS_JS_CODE_CONTEXT=''
  GR_LS_JS_PAREN_STACK=''
  GR_LS_JS_AFTER_CONTROL=0
  case ${1-} in *.tsx|*.jsx|*.astro) GR_LS_JS_JSX_ENABLED=1 ;; *) GR_LS_JS_JSX_ENABLED=0 ;; esac
}

gr_ls_js_push_brace() {
  GR_LS_JS_BRACE_STACK="$1,$GR_LS_JS_BRACE_DEPTH;$GR_LS_JS_BRACE_STACK"
  GR_LS_JS_BRACE_DEPTH=1
  GR_LS_JS_STATE=code
}

gr_ls_js_pop_brace() {
  local frame
  frame=${GR_LS_JS_BRACE_STACK%%;*}
  GR_LS_JS_BRACE_STACK=${GR_LS_JS_BRACE_STACK#*;}
  GR_LS_JS_STATE=${frame%%,*}
  GR_LS_JS_BRACE_DEPTH=${frame#*,}
}

gr_ls_js_after_keyword() {
  local prefix keyword before last
  prefix=$1
  shift
  prefix=${prefix%"${prefix##*[![:space:]]}"}
  for keyword in "$@"; do
    case "$prefix" in
      *"$keyword")
        before=${prefix%"$keyword"}
        before=${before%"${before##*[![:space:]]}"}
        last=${before: -1}
        [[ $last =~ [[:alnum:]_$.#] ]] || return 0
        ;;
    esac
  done
  return 1
}

gr_ls_js_is_control_paren() {
  gr_ls_js_after_keyword "$1" if while for with
}

gr_ls_js_push_paren() {
  local kind
  gr_ls_js_is_control_paren "$1" && kind=control || kind=value
  GR_LS_JS_PAREN_STACK="$kind;$GR_LS_JS_PAREN_STACK"
  GR_LS_JS_AFTER_CONTROL=0
}

gr_ls_js_pop_paren() {
  local kind
  kind=${GR_LS_JS_PAREN_STACK%%;*}
  if [ -n "$GR_LS_JS_PAREN_STACK" ]; then
    GR_LS_JS_PAREN_STACK=${GR_LS_JS_PAREN_STACK#*;}
  fi
  [ "$kind" = control ] && GR_LS_JS_AFTER_CONTROL=1 || GR_LS_JS_AFTER_CONTROL=0
}

gr_ls_js_skip_trivia() {
  local line i length char next
  line=$1
  i=$2
  length=${#line}
  while [ "$i" -lt "$length" ]; do
    char=${line:i:1}
    next=${line:i+1:1}
    if [ -z "${char//[[:space:]]/}" ]; then
      i=$((i + 1))
    elif [ "$char$next" = '/*' ]; then
      i=$((i + 2))
      while [ "$i" -lt "$length" ] && [ "${line:i:2}" != '*/' ]; do i=$((i + 1)); done
      i=$((i + 2))
    elif [ "$char$next" = '//' ]; then
      while [ "$i" -lt "$length" ] && [ "${line:i:1}" != $'\n' ]; do i=$((i + 1)); done
    else
      break
    fi
  done
  GR_LS_JS_SCAN_INDEX=$i
}

gr_ls_js_is_generic_arrow() {
  local line i length char next previous after quote angle paren mode regex_class regex_context control_stack control_kind after_control
  line=$1
  i=$(( $2 + 1 ))
  length=${#line}

  # TSX requires an ambiguity breaker: a comma, default, constraint, or const
  # modifier after the first type-parameter name. A plain `<pre>(x) =>` is JSX
  # text, not a generic arrow, even though the following bytes look arrow-like.
  gr_ls_js_skip_trivia "$line" "$i"
  i=$GR_LS_JS_SCAN_INDEX
  if [ "${line:i:5}" = const ]; then
    after=${line:i+5:1}
    if [ -z "$after" ] || [ -z "${after//[[:space:]]/}" ] || [ "$after${line:i+6:1}" = '/*' ]; then
      i=$((i + 5))
      gr_ls_js_skip_trivia "$line" "$i"
      i=$GR_LS_JS_SCAN_INDEX
    fi
  fi
  char=${line:i:1}
  [[ $char =~ [[:alpha:]_$] ]] || return 1
  i=$((i + 1))
  while [ "$i" -lt "$length" ] && [[ ${line:i:1} =~ [[:alnum:]_$] ]]; do i=$((i + 1)); done
  gr_ls_js_skip_trivia "$line" "$i"
  i=$GR_LS_JS_SCAN_INDEX
  case "${line:i:1}" in ','|'=') ;; *)
    [ "${line:i:7}" = extends ] || return 1
    after=${line:i+7:1}
    [[ ! $after =~ [[:alnum:]_$] ]] || return 1
    ;;
  esac

  i=$(( $2 + 1 ))
  quote=''
  mode=code
  angle=1
  while [ "$i" -lt "$length" ] && [ "$angle" -gt 0 ]; do
    char=${line:i:1}
    next=${line:i+1:1}
    previous=${line:i-1:1}
    case "$mode" in
      code)
        if [ "$char$next" = '//' ]; then mode=line_comment
        elif [ "$char$next" = '/*' ]; then mode=block_comment
        else
          case "$char" in
            "'"|'"'|\`) quote=$char; mode=quote ;;
            '<') angle=$((angle + 1)) ;;
            '>') [ "$previous" = '=' ] || angle=$((angle - 1)) ;;
          esac
        fi
        ;;
      quote)
        if [ "$char" = '\\' ]; then i=$((i + 1)); elif [ "$char" = "$quote" ]; then mode=code; fi
        ;;
      block_comment)
        if [ "$char$next" = '*/' ]; then mode=code; i=$((i + 1)); fi
        ;;
      line_comment) [ "$char" = $'\n' ] && mode=code ;;
    esac
    i=$((i + 1))
  done
  [ "$angle" -eq 0 ] || return 1
  gr_ls_js_skip_trivia "$line" "$i"
  i=$GR_LS_JS_SCAN_INDEX
  [ "${line:i:1}" = '(' ] || return 1
  paren=1
  quote=''
  mode=code
  regex_class=0
  regex_context='('
  control_stack='value;'
  after_control=0
  i=$((i + 1))
  while [ "$i" -lt "$length" ] && [ "$paren" -gt 0 ]; do
    char=${line:i:1}
    next=${line:i+1:1}
    case "$mode" in
      code)
        if [ "$char$next" = '//' ]; then mode=line_comment
        elif [ "$char$next" = '/*' ]; then mode=block_comment
        elif [ "$char" = / ] \
          && { [ "$after_control" -eq 1 ] \
            || gr_ls_js_starts_regex "${regex_context}/${next}" "${#regex_context}"; }; then
          mode=regex
          regex_class=0
          regex_context=${regex_context}v
          after_control=0
        else
          case "$char" in
            "'"|'"'|\`)
              quote=$char; mode=quote; regex_context=${regex_context}v; after_control=0
              ;;
            '(')
              gr_ls_js_is_control_paren "$regex_context" && control_kind=control || control_kind=value
              control_stack="$control_kind;$control_stack"
              paren=$((paren + 1)); regex_context=${regex_context}${char}; after_control=0
              ;;
            ')')
              control_kind=${control_stack%%;*}
              control_stack=${control_stack#*;}
              paren=$((paren - 1)); regex_context=${regex_context}${char}
              [ "$control_kind" = control ] && after_control=1 || after_control=0
              ;;
            *)
              regex_context=${regex_context}${char}
              [ -z "${char//[[:space:]]/}" ] || after_control=0
              ;;
          esac
        fi
        ;;
      quote)
        if [ "$char" = '\\' ]; then i=$((i + 1)); elif [ "$char" = "$quote" ]; then mode=code; fi
        ;;
      block_comment)
        if [ "$char$next" = '*/' ]; then mode=code; i=$((i + 1)); fi
        ;;
      line_comment)
        if [ "$char" = $'\n' ]; then mode=code; regex_context=${regex_context}' '; fi
        ;;
      regex)
        if [ "$char" = '\\' ]; then
          i=$((i + 1))
        elif [ "$regex_class" -eq 1 ]; then
          [ "$char" = ']' ] && regex_class=0
        elif [ "$char" = '[' ]; then
          regex_class=1
        elif [ "$char" = / ]; then
          mode=code
        elif [ "$char" = $'\n' ]; then
          mode=code
        fi
        ;;
    esac
    i=$((i + 1))
  done
  [ "$paren" -eq 0 ] || return 1
  gr_ls_js_skip_trivia "$line" "$i"
  i=$GR_LS_JS_SCAN_INDEX
  [ "${line:i:2}" = '=>' ]
}

# A `<name` token is JSX only at an expression boundary. Looking through the
# closing `>` and parameter list distinguishes generic-arrow spellings from an
# element, including multiline, const, and defaulted type parameters.
gr_ls_js_starts_jsx() {
  local line offset next prefix generic_source generic_offset
  [ "$GR_LS_JS_JSX_ENABLED" -eq 1 ] || return 1
  line=$1
  offset=$2
  next=${line:offset+1:1}
  case "$next" in /|'>'|[[:alpha:]_]) ;; *) return 1 ;; esac
  generic_source=${GR_LS_JS_SOURCE-$line}
  generic_offset=$offset
  [ -n "${GR_LS_JS_SOURCE+x}" ] && generic_offset=$((GR_LS_JS_SOURCE_OFFSET + offset))
  gr_ls_js_is_generic_arrow "$generic_source" "$generic_offset" && return 1
  prefix=${line:0:offset}
  prefix=${prefix%"${prefix##*[![:space:]]}"}
  case "$prefix" in
    ''|*'=>'|*'return'|*'default'|*'yield'|*'await'|*'throw'|*'='|*'('|*'['|*'{'|*','|*':'|*'?'|*';'|*'!'|*'~'|*'+'|*'-'|*'*'|*'/'|*'%'|*'&'|*'|'|*'^') return 0 ;;
    *) return 1 ;;
  esac
}

# Division follows a completed value; a regex literal follows an expression
# boundary. Callers pass code-only context: whitespace and comments are trivia,
# while completed strings, templates, regexes, and JSX nodes use a value token.
# Keeping that context separately prevents a trailing `*/` from turning a
# comment-separated division operator into a supposed regex literal.
gr_ls_js_starts_regex() {
  local line offset next prefix before last
  line=$1
  offset=$2
  next=${line:offset+1:1}
  [ "$next" != '=' ] || return 1
  prefix=${line:0:offset}
  prefix=${prefix%"${prefix##*[![:space:]]}"}
  case "$prefix" in *'++'|*'--') return 1 ;; esac
  if [[ $prefix == *'!' ]]; then
    before=${prefix%?}
    last=${before: -1}
    if [[ $last =~ [[:alnum:]_$] ]]; then return 1; fi
    case "$last" in ')'|']'|'}'|'"'|"'"|\`) return 1 ;; esac
  fi
  gr_ls_js_after_keyword "$prefix" return case delete void typeof yield await \
    throw else do instanceof in of && return 0
  case "$prefix" in
    ''|*'=>'|*'='|*'('|*'['|*'{'|*','|*':'|*'?'|*';'|*'!'|*'~'|*'+'|*'-'|*'*'|*'/'|*'%'|*'&'|*'|'|*'^') return 0 ;;
    *) return 1 ;;
  esac
}

gr_ls_js_start_root() {
  GR_LS_JS_JSX_STACK="code,$GR_LS_JS_JSX_DEPTH;$GR_LS_JS_JSX_STACK"
  GR_LS_JS_JSX_DEPTH=0
  GR_LS_JS_STATE=jsx_tag
  GR_LS_JS_TAG_KIND=$1
  GR_LS_JS_TAG_LAST=''
}

gr_ls_js_finish_root() {
  local frame
  frame=${GR_LS_JS_JSX_STACK%%;*}
  GR_LS_JS_JSX_STACK=${GR_LS_JS_JSX_STACK#*;}
  GR_LS_JS_STATE=${frame%%,*}
  GR_LS_JS_JSX_DEPTH=${frame#*,}
}

# gr_ls_script_syntax_line <line> — preserve JavaScript comments and code while
# replacing quoted/template text and rendered JSX text. Expressions inside
# templates and JSX return to code, keeping real lint comments visible.
gr_ls_script_syntax_line() {
  local line i length char next output context regex_probe
  line=$1
  i=0
  length=${#line}
  output=''
  context=$GR_LS_JS_CODE_CONTEXT
  while [ "$i" -lt "$length" ]; do
    char=${line:i:1}
    next=${line:i+1:1}
    case "$GR_LS_JS_STATE" in
      code)
        if [ "$char$next" = '//' ]; then
          output=${output}${line:i}
          i=$length
        elif [ "$char$next" = '/*' ]; then
          output=${output}'/*'
          GR_LS_JS_STATE=block
          i=$((i + 2))
        elif [ "$char" = / ] \
          && regex_probe="${context}/${next}" \
          && { [ "$GR_LS_JS_AFTER_CONTROL" -eq 1 ] \
            || gr_ls_js_starts_regex "$regex_probe" "${#context}"; }; then
          output="${output} "
          GR_LS_JS_STATE=regex
          GR_LS_JS_REGEX_CLASS=0
          GR_LS_JS_AFTER_CONTROL=0
          context=${context}v
          i=$((i + 1))
        elif [ "$char" = '<' ] && gr_ls_js_starts_jsx "$line" "$i"; then
          output="${output} "
          GR_LS_JS_AFTER_CONTROL=0
          context=${context}v
          i=$((i + 1))
          if [ "$next" = / ]; then
            output="${output} "
            i=$((i + 1))
            gr_ls_js_start_root close
          else
            gr_ls_js_start_root open
          fi
        else
          case "$char" in
            "'") output="${output} "; context=${context}v; GR_LS_JS_AFTER_CONTROL=0; GR_LS_JS_STATE=single; i=$((i + 1)) ;;
            '"') output="${output} "; context=${context}v; GR_LS_JS_AFTER_CONTROL=0; GR_LS_JS_STATE=double; i=$((i + 1)) ;;
            \`) output="${output} "; context=${context}v; GR_LS_JS_AFTER_CONTROL=0; GR_LS_JS_STATE=template; i=$((i + 1)) ;;
            '(')
              gr_ls_js_push_paren "$context"
              output=${output}${char}
              context=${context}${char}
              i=$((i + 1))
              ;;
            ')')
              output=${output}${char}
              context=${context}${char}
              gr_ls_js_pop_paren
              i=$((i + 1))
              ;;
            '{')
              [ "$GR_LS_JS_BRACE_DEPTH" -gt 0 ] && GR_LS_JS_BRACE_DEPTH=$((GR_LS_JS_BRACE_DEPTH + 1))
              output=${output}${char}
              context=${context}${char}
              GR_LS_JS_AFTER_CONTROL=0
              i=$((i + 1))
              ;;
            '}')
              output=${output}${char}
              context=${context}${char}
              GR_LS_JS_AFTER_CONTROL=0
              i=$((i + 1))
              if [ "$GR_LS_JS_BRACE_DEPTH" -gt 0 ]; then
                GR_LS_JS_BRACE_DEPTH=$((GR_LS_JS_BRACE_DEPTH - 1))
                [ "$GR_LS_JS_BRACE_DEPTH" -eq 0 ] && gr_ls_js_pop_brace
              fi
              ;;
            *)
              output=${output}${char}
              context=${context}${char}
              [ -z "${char//[[:space:]]/}" ] || GR_LS_JS_AFTER_CONTROL=0
              i=$((i + 1))
              ;;
          esac
        fi
        ;;
      block)
        output=${output}${char}
        i=$((i + 1))
        if [ "$char$next" = '*/' ]; then
          output=${output}${next}
          GR_LS_JS_STATE=code
          i=$((i + 1))
        fi
        ;;
      regex)
        output="${output} "
        i=$((i + 1))
        if [ "$char" = '\\' ] && [ "$i" -lt "$length" ]; then
          output="${output} "
          i=$((i + 1))
        elif [ "$GR_LS_JS_REGEX_CLASS" -eq 1 ]; then
          [ "$char" = ']' ] && GR_LS_JS_REGEX_CLASS=0
        elif [ "$char" = '[' ]; then
          GR_LS_JS_REGEX_CLASS=1
        elif [ "$char" = / ]; then
          GR_LS_JS_STATE=code
        fi
        ;;
      single|double)
        output="${output} "
        i=$((i + 1))
        if [ "$char" = '\\' ] && [ "$i" -lt "$length" ]; then
          output="${output} "
          i=$((i + 1))
        elif { [ "$GR_LS_JS_STATE" = single ] && [ "$char" = "'" ]; } \
          || { [ "$GR_LS_JS_STATE" = double ] && [ "$char" = '"' ]; }; then
          GR_LS_JS_STATE=code
        fi
        ;;
      template)
        output="${output} "
        i=$((i + 1))
        if [ "$char" = '\\' ] && [ "$i" -lt "$length" ]; then
          output="${output} "
          i=$((i + 1))
        elif [ "$char$next" = '${' ]; then
          output="${output} "
          context=${context}'{'
          i=$((i + 1))
          gr_ls_js_push_brace template
        elif [ "$char" = \` ]; then
          GR_LS_JS_STATE=code
        fi
        ;;
      jsx_text)
        output="${output} "
        i=$((i + 1))
        if [ "$char" = '<' ]; then
          GR_LS_JS_STATE=jsx_tag
          GR_LS_JS_TAG_KIND=open
          GR_LS_JS_TAG_LAST=''
          if [ "$next" = / ]; then
            output="${output} "
            i=$((i + 1))
            GR_LS_JS_TAG_KIND=close
          fi
        elif [ "$char" = '{' ]; then
          context=${context}'{'
          gr_ls_js_push_brace jsx_text
        fi
        ;;
      jsx_tag)
        output="${output} "
        i=$((i + 1))
        case "$char" in
          "'") GR_LS_JS_STATE=jsx_single ;;
          '"') GR_LS_JS_STATE=jsx_double ;;
          '{') context=${context}'{'; gr_ls_js_push_brace jsx_tag ;;
          '>')
            if [ "$GR_LS_JS_TAG_KIND" = close ]; then
              GR_LS_JS_JSX_DEPTH=$((GR_LS_JS_JSX_DEPTH - 1))
              if [ "$GR_LS_JS_JSX_DEPTH" -eq 0 ]; then gr_ls_js_finish_root; else GR_LS_JS_STATE=jsx_text; fi
            elif [ "$GR_LS_JS_TAG_LAST" = / ]; then
              if [ "$GR_LS_JS_JSX_DEPTH" -eq 0 ]; then gr_ls_js_finish_root; else GR_LS_JS_STATE=jsx_text; fi
            else
              GR_LS_JS_JSX_DEPTH=$((GR_LS_JS_JSX_DEPTH + 1))
              GR_LS_JS_STATE=jsx_text
            fi
            ;;
          *) [ -n "${char//[[:space:]]/}" ] && GR_LS_JS_TAG_LAST=$char ;;
        esac
        ;;
      jsx_single|jsx_double)
        output="${output} "
        i=$((i + 1))
        if { [ "$GR_LS_JS_STATE" = jsx_single ] && [ "$char" = "'" ]; } \
          || { [ "$GR_LS_JS_STATE" = jsx_double ] && [ "$char" = '"' ]; }; then
          GR_LS_JS_STATE=jsx_tag
        fi
        ;;
    esac
  done
  # JavaScript regex literals cannot cross a physical newline. Recover code
  # state on malformed input so one bad line cannot hide later directives.
  [ "$GR_LS_JS_STATE" = regex ] && GR_LS_JS_STATE=code
  context=${context}' '
  [ "${#context}" -gt 128 ] && context=${context: -128}
  GR_LS_JS_CODE_CONTEXT=$context
  GR_LS_SANITIZED=$output
}

gr_ls_rust_syntax_reset() {
  GR_LS_RUST_STATE=code
  GR_LS_RUST_COMMENT_DEPTH=0
  GR_LS_RUST_RAW_CLOSE=''
}

# gr_ls_rust_syntax_line <line> — preserve Rust code but erase comments and
# strings and character/byte literals. Rust block comments nest, and raw-string
# delimiters may contain any number of hashes, so both states persist across
# source lines.
gr_ls_rust_syntax_line() {
  local line i length char next output probe hashes close close_length
  line=$1
  i=0
  length=${#line}
  output=''
  while [ "$i" -lt "$length" ]; do
    char=${line:i:1}
    next=${line:i+1:1}
    case "$GR_LS_RUST_STATE" in
      code)
        if [ "$char$next" = '//' ]; then
          while [ "$i" -lt "$length" ]; do output="${output} "; i=$((i + 1)); done
        elif [ "$char$next" = '/*' ]; then
          output="${output}  "
          GR_LS_RUST_STATE=block
          GR_LS_RUST_COMMENT_DEPTH=1
          i=$((i + 2))
        elif [ "$char" = "'" ]; then
          probe=$((i + 1))
          if [ "$next" = '\\' ]; then
            while [ "$probe" -lt "$length" ]; do
              if [ "${line:probe:1}" = '\\' ]; then
                probe=$((probe + 2))
              elif [ "${line:probe:1}" = "'" ]; then
                break
              else
                probe=$((probe + 1))
              fi
            done
          else
            probe=$((probe + 1))
          fi
          if [ "$probe" -lt "$length" ] && [ "${line:probe:1}" = "'" ]; then
            while [ "$i" -le "$probe" ]; do output="${output} "; i=$((i + 1)); done
          else
            output=${output}${char}
            i=$((i + 1))
          fi
        elif [ "$char" = '"' ]; then
          output="${output} "
          GR_LS_RUST_STATE=string
          i=$((i + 1))
        elif [ "$char" = r ]; then
          probe=$((i + 1))
          hashes=''
          while [ "$probe" -lt "$length" ] && [ "${line:probe:1}" = '#' ]; do
            hashes=${hashes}'#'
            probe=$((probe + 1))
          done
          if [ "$probe" -lt "$length" ] && [ "${line:probe:1}" = '"' ]; then
            GR_LS_RUST_RAW_CLOSE='"'${hashes}
            GR_LS_RUST_STATE=raw
            while [ "$i" -le "$probe" ]; do output="${output} "; i=$((i + 1)); done
          else
            output=${output}${char}
            i=$((i + 1))
          fi
        else
          output=${output}${char}
          i=$((i + 1))
        fi
        ;;
      block)
        if [ "$char$next" = '/*' ]; then
          GR_LS_RUST_COMMENT_DEPTH=$((GR_LS_RUST_COMMENT_DEPTH + 1))
          output="${output}  "
          i=$((i + 2))
        elif [ "$char$next" = '*/' ]; then
          GR_LS_RUST_COMMENT_DEPTH=$((GR_LS_RUST_COMMENT_DEPTH - 1))
          output="${output}  "
          i=$((i + 2))
          [ "$GR_LS_RUST_COMMENT_DEPTH" -eq 0 ] && GR_LS_RUST_STATE=code
        else
          output="${output} "
          i=$((i + 1))
        fi
        ;;
      string)
        output="${output} "
        i=$((i + 1))
        if [ "$char" = '\\' ] && [ "$i" -lt "$length" ]; then
          output="${output} "
          i=$((i + 1))
        elif [ "$char" = '"' ]; then
          GR_LS_RUST_STATE=code
        fi
        ;;
      raw)
        close=$GR_LS_RUST_RAW_CLOSE
        close_length=${#close}
        if [ "${line:i:close_length}" = "$close" ]; then
          while [ "$close_length" -gt 0 ]; do
            output="${output} "
            i=$((i + 1))
            close_length=$((close_length - 1))
          done
          GR_LS_RUST_STATE=code
          GR_LS_RUST_RAW_CLOSE=''
        else
          output="${output} "
          i=$((i + 1))
        fi
        ;;
    esac
  done
  GR_LS_SANITIZED=$output
}
