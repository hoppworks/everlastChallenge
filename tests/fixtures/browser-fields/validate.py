"""Bounded static fixture contract, not browser/native acceptance."""
from html.parser import HTMLParser
import re

def check(condition,message):
    if not condition:
        raise ValueError(message)

class Page(HTMLParser):
    def __init__(self):
        super().__init__()
        self.nodes={}
        self.cases={}
        self.labels=set()
        self.csp=None

    def handle_starttag(self,tag,attrs):
        attrs=dict(attrs)
        for key in attrs:
            check(not key.startswith("on"),"inline event attributes forbidden")
        for key in ("href","src","action","formaction","poster"):
            if key in attrs:
                check(key=="href" and attrs[key].startswith("#"),"external/resource/navigation target forbidden")
        check(tag not in ("iframe","object","embed","form","base","link","img","video","audio"),"resource/form element forbidden")
        if tag=="meta" and attrs.get("http-equiv","").lower()=="content-security-policy":
            check(self.csp is None,"duplicate CSP")
            self.csp=attrs.get("content","")
        if tag=="label":
            self.labels.add(attrs.get("for"))
        if "id" in attrs:
            check(attrs["id"] not in self.nodes,"duplicate DOM id")
            self.nodes[attrs["id"]]=(tag,attrs)
        if "data-case" in attrs:
            check(attrs["data-case"] not in self.cases,"duplicate HTML case")
            self.cases[attrs["data-case"]]=attrs["id"]

def validate(html,matrix):
    check(len(html.encode("utf-8"))<=128*1024,"fixture exceeds bound")
    page=Page()
    page.feed(html)
    check(page.csp is not None,"CSP missing")
    directives={part.strip().split()[0]:" ".join(part.strip().split()[1:])
                for part in page.csp.split(";") if part.strip()}
    check(all(directives.get(k)=="'none'" for k in
              ("default-src","connect-src","img-src","font-src","frame-src","object-src","base-uri","form-action")),
          "network/resource CSP weakened")
    check(directives.get("script-src")==directives.get("style-src")=="'unsafe-inline'","only local inline code/style allowed")
    expected={*(f"E{i:02}" for i in range(1,5)),*(f"N{i:02}" for i in range(1,8)),
              *(f"D{i:02}" for i in range(1,5)),"L01","L02","S01"}
    check(set(page.cases)==expected,"missing/unexpected HTML field case")
    for cid,field in page.cases.items():
        tag,attrs=page.nodes[field]
        if tag in ("input","textarea"):
            check(field in page.labels,"missing visible field label")
        if attrs.get("role") in ("textbox","listbox"):
            check(attrs.get("aria-labelledby") in page.nodes,"missing accessible label")
    _,aria=page.nodes["aria-input"]
    check(aria.get("aria-readonly")=="true" and "readonly" not in aria and "disabled" not in aria,"ARIA-only case disguised")
    _,password=page.nodes["password-input"]
    check(password.get("type")=="password" and "value" not in password,"password case prefilled")
    scripts=re.findall(r"<script[^>]*>([\s\S]*?)</script>",html)
    check(len(scripts)==1,"one local fixture script required")
    code=scripts[0]
    check(not re.search(r"\b(fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon|localStorage|sessionStorage|indexedDB|console|eval|Function)\b|\.\s*(value|clipboard)\b",code),
          "input reading/telemetry/dynamic evaluation forbidden")
    check(matrix.get("schemaVersion")==1 and matrix.get("nativeExecutionPerformed") is False and
          matrix.get("qualified") is False and matrix.get("delayMillis")==5000,"prospective scope changed")
    rows=matrix["cases"]
    check(len(rows)==20 and {r["id"] for r in rows}==expected|{"X01","A21a-transport"},"workflow matrix differs")
    check(all(r["status"]=="not-run" and r["observations"] is None and r["element"] in page.nodes
              and r["requirements"] and r["actions"] and r["expected"] for r in rows),"matrix contains claims/missing target")
    return dict(htmlCases=18,workflows=20,nativeExecutionPerformed=False)
