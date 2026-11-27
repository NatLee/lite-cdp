import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { fakeApi } from "../test/fakeApi";
import { Connections } from "./Connections";
import { Authorize } from "./Authorize";
import { ProviderButtons } from "./ProviderButtons";
import { safeAuthorizeNext, providerErrorMessage } from "./navigation";

const summary = {days:7,from:"2026-11-21",to:"2026-11-27",timezone:"UTC",attempts:0,successes:0,failures:0,pending:0};
const providers = {items:[{provider:"github",name:"GitHub",enabled:false,login_start_url:"/oauth/login/github",disabled_reason:"GitHub 尚未設定 client"},{provider:"google",name:"Google",enabled:false,login_start_url:"/oauth/login/google",disabled_reason:"Google 尚未設定 client"}]};
function routes() {
 return {
  "GET /access/providers":()=>providers,
  "GET /access/overview":()=>({identities:[],credentials:[],grants:[],summary}),
  "GET /access/usage":()=>({...summary,transport:null,totals:summary,daily:[]}),
  "GET /access/events":()=>({items:[],next_cursor:null}),
  "GET /access/clients":()=>({items:[]}),
 };
}

test("unconfigured providers explain their disabled state instead of pretending to connect", async()=>{
 fakeApi(routes());render(<ProviderButtons />);
 expect(await screen.findByRole("button",{name:"使用 GitHub 登入"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"使用 Google 登入"})).toBeDisabled();
 expect(screen.getByText("GitHub 尚未設定 client")).toBeInTheDocument();
});

test("new key sends explicit audiences; its raw value disappears when dismissed", async()=>{
 const credential={id:"key-1",name:"研究終端",prefix:"ak_123",scopes:["reports:read"],audiences:["api"],created_at:"2026-11-27T00:00:00Z",expires_at:"2026-12-27T00:00:00Z",last_used_at:null,revoked_at:null};
 const calls=fakeApi({...routes(),"POST /access/credentials":()=>({credential,token:"only-once-secret"})});
 render(<Connections isAdmin={false}/>);
 await screen.findByRole("heading",{name:"連線與存取"});
 await userEvent.type(screen.getByLabelText("金鑰名稱"),"研究終端");
 await userEvent.click(screen.getByLabelText("MCP 工具"));
 await userEvent.click(screen.getByRole("button",{name:"建立 API 金鑰"}));
 const secret=await screen.findByRole("dialog",{name:"一次性 API 金鑰"});
 expect(within(secret).getByLabelText("金鑰原值")).toHaveValue("only-once-secret");
 expect(calls.find(c=>c.method==="POST"&&c.path==="/access/credentials")?.body).toEqual({name:"研究終端",scopes:["reports:read"],audiences:["api"],expires_in_days:30});
 await userEvent.click(within(secret).getByRole("button",{name:"關閉並清除金鑰原值"}));
 expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
 expect(screen.getByLabelText("金鑰名稱")).toHaveFocus();
 expect(localStorage.getItem("only-once-secret")).toBeNull();
 expect(calls.some(c=>c.path==="/access/clients")).toBe(false);
});

test("usage uses server numbers and revocation reloads actual credential state",async()=>{
 let revoked=false;
 const credential={id:"key-1",name:"終端",prefix:"ak_123",scopes:["reports:read"],audiences:["api","mcp"],created_at:"2026-11-27T00:00:00Z",expires_at:"2026-12-27T00:00:00Z",last_used_at:null};
 const calls=fakeApi({...routes(),"GET /access/overview":()=>({identities:[],credentials:[{...credential,revoked_at:revoked?"2026-11-27T01:00:00Z":null}],grants:[],summary}),"GET /access/usage":()=>({...summary,transport:"api",totals:{attempts:3,successes:2,failures:1,pending:0},daily:[{date:"2026-11-27",attempts:3,successes:2,failures:1,pending:0}]}),"DELETE /access/credentials/key-1":()=>{revoked=true;return undefined;}});
 render(<Connections isAdmin={false}/>);
 expect(await screen.findByText("嘗試 3")).toBeInTheDocument();
 await userEvent.click(screen.getByRole("button",{name:"撤銷金鑰 終端"}));
 expect(await screen.findByText("已撤銷")).toBeInTheDocument();
 expect(calls.some(c=>c.method==="DELETE"&&c.path.endsWith("key-1"))).toBe(true);
});

test("admin client registration sends only the supported public OAuth contract",async()=>{
 const calls=fakeApi({...routes(),"POST /access/clients":()=>({client_id:"client-1",client_name:"報表工具",redirect_uris:["http://127.0.0.1:9000/callback"]})});
 render(<Connections isAdmin/>);
 await screen.findByLabelText("應用名稱");
 await userEvent.type(screen.getByLabelText("應用名稱"),"報表工具");
 await userEvent.type(screen.getByLabelText("回呼網址（每行一個）"),"http://127.0.0.1:9000/callback");
 await userEvent.click(screen.getByRole("button",{name:"註冊 OAuth 應用"}));
 expect(calls.find(c=>c.method==="POST"&&c.path==="/access/clients")?.body).toEqual({client_name:"報表工具",redirect_uris:["http://127.0.0.1:9000/callback"],token_endpoint_auth_method:"none",grant_types:["authorization_code","refresh_token"],response_types:["code"]});
});

test("consent displays immutable server data and posts only an approve or deny decision",async()=>{
 fakeApi({"GET /access/consents/request-1":()=>({request_id:"request-1",client_name:"研究工具",scopes:["reports:read"],resource:"https://example.test/mcp",expires_at:"2026-11-27T01:00:00Z",user:{id:1,username:"member",display_name:"研究員"}})});
 render(<Authorize requestId="request-1"/>);
 expect(await screen.findByRole("heading",{name:"授權 研究工具"})).toBeInTheDocument();
 const form=screen.getByRole("button",{name:"同意授權"}).closest("form")!;
 expect(form).toHaveAttribute("method","post");
 expect(form).toHaveAttribute("action","/oauth/consent/request-1");
 expect(form.querySelectorAll("input,select,textarea")).toHaveLength(0);
 expect(screen.getByRole("button",{name:"拒絕"})).toHaveAttribute("value","deny");
});

test("login continuation accepts only the local authorization route",()=>{
 expect(safeAuthorizeNext("?next=%2Foauth%2Fauthorize%3Fclient_id%3Dknown")).toBe("/oauth/authorize?client_id=known");
 for(const next of ["https://evil.test","//evil.test","/oauth/authorize-evil?x=1","/connections","/\\evil"]){expect(safeAuthorizeNext("?next="+encodeURIComponent(next))).toBeNull();}
});

test("loading another audit page preserves earlier records",async()=>{
 const event=(id:string,operation:string)=>({id,request_id:id,at:"2026-11-27T00:00:00Z",kind:"operation",transport:"api",operation,user_id:1,client_id:null,credential_id:"key-1",status:200,outcome:"success",duration_ms:12});
 fakeApi({...routes(),"GET /access/events":(_body,url)=>url.searchParams.get("cursor")?{items:[event("2","second_operation")],next_cursor:null}:{items:[event("1","first_operation")],next_cursor:"cursor-1"}});
 render(<Connections isAdmin={false}/>);
 await userEvent.click(await screen.findByRole("button",{name:"載入更多紀錄"}));
 expect(await screen.findByText("second_operation",{exact:false})).toBeInTheDocument();
 expect(screen.getByText("first_operation",{exact:false})).toBeInTheDocument();
});


test("provider callback errors explain account linking without reflecting arbitrary text",()=>{
 expect(providerErrorMessage("?access_error=identity_not_linked")).toContain("原帳號登入");
 expect(providerErrorMessage("?access_error=%3Cscript%3Eevil%3C%2Fscript%3E")).toBeNull();
});

test("admin can inspect a specific account without changing whose credentials are managed",async()=>{
 const usageQueries: URL[] = [], overviewQueries: URL[] = [];
 fakeApi({...routes(),"GET /access/usage":(_body,url)=>{usageQueries.push(url);return {...summary,transport:null,totals:summary,daily:[]};},"GET /access/overview":(_body,url)=>{overviewQueries.push(url);return {identities:[],credentials:[],grants:[],summary};}});
 render(<Connections isAdmin/>);
 await userEvent.selectOptions(await screen.findByLabelText("查看帳號"),"user");
 await userEvent.type(screen.getByLabelText("帳號 ID"),"17");
 await userEvent.click(screen.getByRole("button",{name:"查看此帳號"}));
 await screen.findByText("這段期間尚無工具執行，統計為 0。");
 expect(usageQueries.some(url=>url.searchParams.get("user_id")==="17")).toBe(true);
 expect(overviewQueries.every(url=>!url.searchParams.has("user_id"))).toBe(true);
});

test("changing audit filters does not refetch identity, credential, or client management data",async()=>{
 const calls=fakeApi(routes());render(<Connections isAdmin/>);
 await userEvent.selectOptions(await screen.findByLabelText("通道"),"mcp");
 expect(calls.filter(c=>c.path==="/access/overview")).toHaveLength(1);
 expect(calls.filter(c=>c.path==="/access/clients")).toHaveLength(1);
 expect(calls.filter(c=>c.path==="/access/usage")).toHaveLength(2);
});

test("personal event access is a separate explicit choice",async()=>{
 const calls=fakeApi({...routes(),"POST /access/credentials":()=>({credential:{id:"key-2"},token:"once"})});
 render(<Connections isAdmin={false}/>);await screen.findByLabelText("金鑰名稱");
 const choice=screen.getByRole("checkbox",{name:/另允許 people:read/});expect(choice).not.toBeChecked();
 await userEvent.click(choice);await userEvent.type(screen.getByLabelText("金鑰名稱"),"個人事件工具");await userEvent.click(screen.getByRole("button",{name:"建立 API 金鑰"}));await screen.findByRole("dialog");
 expect(calls.find(c=>c.method==="POST"&&c.path==="/access/credentials")?.body).toMatchObject({scopes:["reports:read","people:read"]});
});
