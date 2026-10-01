#!/usr/bin/env bun
// @bun
class o extends Error{code;stdout;constructor(n,e="",t=""){super(e);this.name="CliExit",this.code=n,this.stdout=t}}function c(n,e,t){let r=e[t+1];if(r===void 0)throw new o(1,`${n}: ${e[t]??""} needs a value`);return r}var L=/^[ \t\n\r]*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?[ \t\n\r]*$/;function d(n,e,t){let r=Number(t);if(!L.test(t)||!Number.isFinite(r))throw new o(2,`${n}: ${e} must be a number`);return r}function M(n){return n instanceof Error&&"code"in n&&n.code==="EPIPE"}async function m(n){if(n.length===0)return;try{await Bun.write(Bun.stdout,n)}catch(e){if(M(e))throw new o(141);throw e}}async function w(n){if(n!=="")await Bun.write(Bun.stderr,n.endsWith(`
`)?n:`${n}
`)}async function O(n){let e=n.code;try{await m(n.stdout)}catch(t){if(!(t instanceof o))throw t;e=t.code}return await w(n.message),e}async function y(n){let e;try{e=await n()}catch(t){if(t instanceof o)e=await O(t);else await w(t instanceof Error?t.message:String(t)),e=1}process.exit(e)}function x(n){return`${JSON.stringify(n,null,2)}
`}function R(n){try{return{ok:!0,value:JSON.parse(n)}}catch{return{ok:!1}}}function N(n,e){let t=R(e);if(!t.ok)throw new o(5,`${n}: response is not JSON`);return t.value}function f(n,e,t=(r)=>r){if(e.trim()==="")return"";return x(t(N(n,e)))}function b(n){return n instanceof Error?n.message:String(n)}async function F(n,e,t){try{return await fetch(e,{...t,redirect:"manual"})}catch(r){throw new o(1,`${n}: request failed: ${b(r)}`)}}async function j(n,e){try{return await e()}catch(t){throw new o(1,`${n}: request failed: ${b(t)}`)}}async function E(n,e){let t={method:e.method??"GET",headers:{...e.headers}};if(e.payload!==void 0)t.body=e.payload;else if(e.body!==void 0)t.body=JSON.stringify(e.body);let r=await F(n,e.url,t),{status:s}=r,a=await j(n,()=>r.text());if(s>=400){let u=e.json===!1?void 0:R(a),i=u?.ok===!0?x(u.value):a;throw new o(22,`${n}: HTTP ${s} from ${e.url}`,i)}return a}var q=["title","url","publishedDate","author","highlights","text","summary"];function A(n){return typeof n==="object"&&n!==null&&!Array.isArray(n)}function Y(n){let e=A(n)?n:{},t={};for(let r of q){let s=e[r];if(s!==void 0&&s!==null&&s!=="")t[r]=s}return t}function S(n){let e=A(n)?n:{},t=Array.isArray(e.results)?e.results:[];return{requestId:e.requestId??null,results:t.map(Y)}}var C=`Exa Search CLI

Usage: search.sh <command> [options]

Environment:
  EXA_API_KEY  Required. Exa API key.

Commands:
  search   Search the web (default if no command given)
  crawl    Extract content from URLs
  similar  Find pages similar to a URL

Run 'search.sh <command>' with no args for command-specific help.`,k=`Usage: search.sh search -q <query> [options]
  -n, --num-results  Number of results (default: 10)
  -t, --type         instant|fast|auto|deep-lite|deep|deep-reasoning (default: auto)
  -c, --category     company|research paper|news|personal site|financial report|people
  --include-domains  Comma-separated domains to include
  --exclude-domains  Comma-separated domains to exclude
  --start-date       Start published date (YYYY-MM-DD)
  --end-date         End published date (YYYY-MM-DD)
  --include-text     Text that must appear in results
  --exclude-text     Text to exclude from results
  --highlights N     Max highlight chars (default: 4000)
  --with-text        Include full text in results
  --lean             Strip image/favicon/subpages/entities for AI prompts`,U=`Usage: search.sh crawl <url> [url...] [-m max_chars]
  Extracts content from one or more URLs
  -m, --max-chars  Max characters per page (default: 3000)`,T=`Usage: search.sh similar <url> [-n num_results]
  Finds pages similar to the given URL
  -n, --num-results  Number of results (default: 10)
  --highlights N     Max highlight chars (default: 4000)`;var l="exa-search",P={"-q":"query","--query":"query","-n":"numResults","--num-results":"numResults","-t":"type","--type":"type","-c":"category","--category":"category","--include-domains":"includeDomains","--exclude-domains":"excludeDomains","--start-date":"startDate","--end-date":"endDate","--include-text":"includeText","--exclude-text":"excludeText","--highlights":"highlights"};function G(n){let e={values:new Map,spelled:new Map,switches:new Set};for(let t=0;t<n.length;t+=1){let r=n[t]??"",s=Object.hasOwn(P,r)?P[r]:void 0;if(s!==void 0)e.values.set(s,c(l,n,t)),e.spelled.set(s,r),t+=1;else if(r==="--with-text"||r==="--lean")e.switches.add(r);else if((e.values.get("query")??"")==="")e.values.set("query",r);else throw new o(1,`Unknown option: ${r}`)}return e}function D(n,e,t){let r=n.values.get(e);return r===void 0?t:d(l,n.spelled.get(e)??e,r)}function p(n){let e=G(n),{values:t}=e,r=t.get("query")??"";if(r==="")throw new o(1,k);let s={highlights:{maxCharacters:D(e,"highlights",4000)}};if(e.switches.has("--with-text"))s.text=!0;let a={query:r,type:t.get("type")??"auto",numResults:D(e,"numResults",10),contents:s},u=(i,_)=>{let g=t.get(i);if(g!==void 0&&g!=="")_(g)};return u("category",(i)=>a.category=i),u("includeDomains",(i)=>a.includeDomains=i.split(",")),u("excludeDomains",(i)=>a.excludeDomains=i.split(",")),u("startDate",(i)=>a.startPublishedDate=`${i}T00:00:00.000Z`),u("endDate",(i)=>a.endPublishedDate=`${i}T00:00:00.000Z`),u("includeText",(i)=>a.includeText=[i]),u("excludeText",(i)=>a.excludeText=[i]),{endpoint:"search",body:a,lean:e.switches.has("--lean")}}function I(n){let e="3000",t="-m",r=[];for(let a=0;a<n.length;a+=1){let u=n[a]??"";if(u==="-m"||u==="--max-chars")e=c(l,n,a),t=u,a+=1;else r.push(u)}if(r.length===0)throw new o(1,U);let s=d(l,t,e);return{endpoint:"contents",body:{urls:r,text:!0,highlights:{maxCharacters:s}},lean:!1}}function v(n){let e={url:"",numResults:"10",highlights:"4000",numFlag:"-n",highlightsFlag:"--highlights"};for(let r=0;r<n.length;r+=1){let s=n[r]??"";if(s==="-n"||s==="--num-results")e.numResults=c(l,n,r),e.numFlag=s,r+=1;else if(s==="--highlights")e.highlights=c(l,n,r),r+=1;else if(e.url==="")e.url=s;else throw new o(1,`Unknown option: ${s}`)}if(e.url==="")throw new o(1,T);return{endpoint:"findSimilar",body:{url:e.url,numResults:d(l,e.numFlag,e.numResults),contents:{highlights:{maxCharacters:d(l,e.highlightsFlag,e.highlights)}}},lean:!1}}var h="exa-search",B="https://api.exa.ai";function H(n){let[e="",...t]=n;switch(e){case"":case"-h":case"--help":throw new o(1,C);case"search":return p(t);case"crawl":return I(t);case"similar":return v(t);default:return p(n)}}async function J(){let n=process.env.EXA_API_KEY??"";if(n==="")throw new o(1,`${h}: EXA_API_KEY unset`);let e=H(process.argv.slice(2)),t=await E(h,{url:`${B}/${e.endpoint}`,method:"POST",headers:{"x-api-key":n,"Content-Type":"application/json",Accept:"application/json"},body:e.body});return await m(e.lean?f(h,t,S):f(h,t)),0}await y(J);
