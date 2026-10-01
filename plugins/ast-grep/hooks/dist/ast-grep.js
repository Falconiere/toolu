#!/usr/bin/env bun
// @bun
import{spawnSync as u}from"child_process";import{existsSync as c,statSync as l}from"fs";var g=`Usage: ast-grep.js <subcommand> [args...]

Subcommands:
  search <pattern> [--lang <L>] [flags]  Pattern search (--color never)
  files <pattern> [--lang <L>] [flags]   File paths only (--files-with-matches)
  scan <yaml> [flags]                    Rule-based scan (--report-style short --max-results 50)
  debug <pattern> [--lang <L>]           Debug pattern AST (--debug-query=pattern)

--lang is required for search/files/debug, but is auto-inferred from the
first path argument's extension if not supplied.

Pass-through flags: --globs <pat>, -A/-B/-C <N>, --max-results N
`,i={ts:"typescript",tsx:"tsx",js:"javascript",mjs:"javascript",cjs:"javascript",jsx:"jsx",rs:"rust",py:"python",go:"go",rb:"ruby",java:"java"};function o(e){try{return l(e).isFile()}catch{return!1}}function f(e){let a=e.some((t)=>t==="--lang"||t==="-l"||t.startsWith("--lang=")||t.startsWith("-l=")),r=e.find((t)=>c(t));if(a||r===void 0||!o(r))return[];let n=r.slice(r.lastIndexOf(".")+1),s=Object.hasOwn(i,n)?i[n]:void 0;return s===void 0?[]:["--lang",s]}function p(e,a){let[r="",...n]=a;switch(e){case"search":case"files":case"debug":{if(r==="")return{error:`${e} requires a pattern`};let s={search:[],files:["--files-with-matches"],debug:["--debug-query=pattern"]}[e];return["run","--pattern",r,...s,"--color","never",...f(n),...n]}case"scan":{if(r==="")return{error:"scan requires inline YAML or rule file path"};return["scan",...o(r)?["--rule",r]:["--inline-rules",r],"--report-style","short","--max-results","50","--color","never",...n]}default:return{error:""}}}function d(e){let a=Bun.which("sg")??Bun.which("ast-grep");if(a===null)return 0;let[r="",...n]=e,s=p(r,n);if(!Array.isArray(s)){if(s.error==="")process.stdout.write(g);else process.stderr.write(`ast-grep.js: ${s.error}
`);return 1}let t=u(a,s,{stdio:"inherit"});if(t.error!==void 0)return process.stderr.write(`ast-grep.js: ${t.error.message}
`),127;return t.status??1}process.exitCode=d(process.argv.slice(2));
