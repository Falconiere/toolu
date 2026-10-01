#!/usr/bin/env bun
// @bun
import{existsSync as s,readFileSync as u,statSync as l}from"fs";function c(n){return typeof n==="object"&&n!==null&&"kind"in n&&typeof n.kind==="string"&&"returned"in n&&typeof n.returned==="number"&&"full"in n&&typeof n.full==="number"}function f(n){try{let r=JSON.parse(n);return c(r)?r:void 0}catch{return}}function p(n){let r=new Map;for(let e of n){let t=r.get(e.kind)??{kind:e.kind,returned:0,full:0,n:0};r.set(e.kind,{...t,returned:t.returned+e.returned,full:t.full+e.full,n:t.n+1})}let d=[...r.keys()].toSorted().flatMap((e)=>r.get(e)??[]),o=d.map((e)=>e.kind==="read"&&e.full>0?`${e.kind}: returned=${e.returned} full=${e.full} saved=${Math.floor((e.full-e.returned)*100/e.full)}% (n=${e.n})`:`${e.kind}: returned=${e.returned} (n=${e.n})`),i=d.reduce((e,t)=>e+t.returned,0);return[...o,`TOTAL returned: ${i} bytes (~${Math.floor(i/4)} tok)`].join(`
`)}function g(n){let r=n[0]??"";if(r===""||!s(r)||!l(r).isFile())return process.stderr.write(`usage: byte-savings-report.js <ledger.jsonl>
`),1;let d=[],o=u(r,"utf8").split(`
`);for(let[i,e]of o.entries()){if(e.trim()==="")continue;let t=f(e);if(t===void 0)return process.stderr.write(`byte-savings-report: ${r}:${i+1}: invalid ledger line
`),1;d.push(t)}return process.stdout.write(`${p(d)}
`),0}process.exitCode=g(process.argv.slice(2));
