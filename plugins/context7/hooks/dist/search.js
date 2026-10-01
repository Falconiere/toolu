#!/usr/bin/env bun
// @bun
class o extends Error{code;stdout;constructor(r,n="",e=""){super(n);this.name="CliExit",this.code=r,this.stdout=e}}function u(r,n,e){let t=n[e+1];if(t===void 0)throw new o(1,`${r}: ${n[e]??""} needs a value`);return t}function P(r){return r instanceof Error&&"code"in r&&r.code==="EPIPE"}async function d(r){if(r.length===0)return;try{await Bun.write(Bun.stdout,r)}catch(n){if(P(n))throw new o(141);throw n}}async function f(r){if(r!=="")await Bun.write(Bun.stderr,r.endsWith(`
`)?r:`${r}
`)}async function U(r){let n=r.code;try{await d(r.stdout)}catch(e){if(!(e instanceof o))throw e;n=e.code}return await f(r.message),n}async function y(r){let n;try{n=await r()}catch(e){if(e instanceof o)n=await U(e);else await f(e instanceof Error?e.message:String(e)),n=1}process.exit(n)}function h(r){return`${JSON.stringify(r,null,2)}
`}function m(r){try{return{ok:!0,value:JSON.parse(r)}}catch{return{ok:!1}}}function j(r,n){let e=m(n);if(!e.ok)throw new o(5,`${r}: response is not JSON`);return e.value}function g(r,n,e=(t)=>t){if(n.trim()==="")return"";return h(e(j(r,n)))}function p(r){return r instanceof Error?r.message:String(r)}async function O(r,n,e){try{return await fetch(n,{...e,redirect:"manual"})}catch(t){throw new o(1,`${r}: request failed: ${p(t)}`)}}async function v(r,n){try{return await n()}catch(e){throw new o(1,`${r}: request failed: ${p(e)}`)}}async function w(r,n){let e={method:n.method??"GET",headers:{...n.headers}};if(n.payload!==void 0)e.body=n.payload;else if(n.body!==void 0)e.body=JSON.stringify(n.body);let t=await O(r,n.url,e),{status:i}=t,s=await v(r,()=>t.text());if(i>=400){let a=n.json===!1?void 0:m(s),c=a?.ok===!0?h(a.value):s;throw new o(22,`${r}: HTTP ${i} from ${n.url}`,c)}return s}function T(r){return encodeURIComponent(r).replaceAll(/[!'()*]/g,(n)=>`%${n.charCodeAt(0).toString(16).toUpperCase()}`)}function b(r){if(r.length===0)return"";return`?${r.map(([n,e])=>`${n}=${T(e)}`).join("&")}`}var x=`Context7 CLI \u2014 Library Documentation Lookup

Usage: search.sh <command> [options]

Environment:
  CONTEXT7_API_KEY  Optional. If set and starts with 'ctx7sk', sent as Bearer token.

Commands:
  search  Find libraries by name (resolve library ID)
  docs    Query documentation for a library

Workflow:
  1. search.sh search <library>    # find the library ID
  2. search.sh docs <id> <query>   # query its docs

Run 'search.sh <command>' with no args for command-specific help.`,C=`Usage: search.sh search <library> [query]
  Searches for libraries matching the name

  -l, --library  Library name (required)
  -q, --query    Context for ranking results

Examples:
  search.sh search react
  search.sh search tokio "async runtime for Rust"`,R=`Usage: search.sh docs <library_id> <query>
  Retrieves documentation context for a library

  -l, --library-id  Context7 library ID, e.g. /vercel/next.js (required)
  -q, --query       Your question (required)
  -t, --type        Output format: json|txt (default: json; txt is LLM-prompt-ready)
  --fast            Skip LLM reranking, return top vector-search hits (lower latency)

Examples:
  search.sh docs /vercel/next.js "app router file conventions"
  search.sh docs /tokio-rs/tokio "spawn async tasks" -t txt

Tip: Run 'search.sh search <name>' first to find the library ID.`;var E="context7";function k(r,n,e){let t={first:"",second:"",type:"json",fast:!1};for(let i=0;i<r.length;i+=1){let s=r[i]??"",a=Object.hasOwn(n,s)?n[s]:void 0;if(a!==void 0){let c=u(E,r,i);if(a===1)t.first=c;else t.second=c;i+=1}else if(e&&(s==="-t"||s==="--type"))t.type=u(E,r,i),i+=1;else if(e&&s==="--fast")t.fast=!0;else if(t.first==="")t.first=s;else if(t.second==="")t.second=s;else throw new o(1,`Unknown option: ${s}`)}return t}function l(r){let{first:n,second:e}=k(r,{"-l":1,"--library":1,"-q":2,"--query":2},!1);if(n==="")throw new o(1,C);return{endpoint:"libs/search",params:[["libraryName",n],["query",e===""?n:e]],json:!0}}function A(r){let n=k(r,{"-l":1,"--library-id":1,"-q":2,"--query":2},!0);if(n.first===""||n.second==="")throw new o(1,R);let e=[["libraryId",n.first],["query",n.second],["type",n.type]];if(n.fast)e.push(["fast","true"]);return{endpoint:"context",params:e,json:n.type==="json"}}var S="context7",q="https://context7.com/api/v2";function I(r){let[n="",...e]=r;switch(n){case"":case"-h":case"--help":throw new o(1,x);case"search":return l(e);case"docs":return A(e);default:return l(r)}}async function L(){let r=I(process.argv.slice(2)),n={Accept:"application/json"},e=process.env.CONTEXT7_API_KEY??"";if(e.startsWith("ctx7sk"))n.Authorization=`Bearer ${e}`;let t=await w(S,{url:`${q}/${r.endpoint}${b(r.params)}`,headers:n,json:r.json});return await d(r.json?g(S,t):t),0}await y(L);
