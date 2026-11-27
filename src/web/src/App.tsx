import { Register } from "./pages/Register";
import { Connections } from "./access/Connections";
import { Authorize } from "./access/Authorize";
import { ContinueAuthorization } from "./access/ContinueAuthorization";
import { safeAuthorizeNext, providerErrorMessage } from "./access/navigation";
import { Globe } from "lucide-react";
import { Link, Navigate, Route, Routes, useLocation } from "react-router";
import { Layout, PageBody, SiteScope, useSites } from "./components/Layout";
import { Empty, ErrorBanner, Loading } from "./components/ui";
import { useApp, useUser } from "./context";
import { Install } from "./pages/Install";
import { Labels } from "./pages/Labels";
import { Login } from "./pages/Login";
import { FirstLogin } from "./pages/FirstLogin";
import { OverviewPage } from "./pages/Overview";
import { Pages } from "./pages/Pages";
import { People } from "./pages/People";
import { PersonPage } from "./pages/Person";
import { SegmentEditor } from "./pages/SegmentEditor";
import { Segments } from "./pages/Segments";
import { Settings } from "./pages/Settings";
import { Sites } from "./pages/Sites";

/** 首頁：有網站就直接進第一個的總覽。 */
function Home() {
  const { sites } = useSites();
  const admin = useUser().role === "admin";
  if (sites.length > 0) return <Navigate to={`/s/${sites[0].id}/overview`} replace />;
  return (
    <PageBody>
      <div className="card">
        <Empty icon={Globe} title={admin ? "先新增一個網站" : "你還沒有可以看的網站"}>
          {admin ? (
            <>
              每個網站會有一段追蹤程式碼，貼進網頁之後就會開始記錄訪客看了什麼。
              <div className="mt-4">
                <Link to="/sites" className="btn btn-primary">
                  <Globe className="size-4" /> 新增網站
                </Link>
              </div>
            </>
          ) : (
            "請管理員把你加進網站的成員，就能看到那個網站的資料。"
          )}
        </Empty>
      </div>
    </PageBody>
  );
}

export function App() {
  const { auth, authError } = useApp();
  const location=useLocation();
  const next=safeAuthorizeNext(location.search);
  if (!auth) {
    return (
      <div className="mx-auto max-w-md px-4 py-10">
        {authError ? <ErrorBanner message={`連不上伺服器：${authError}`} /> : <Loading />}
      </div>
    );
  }
  if (!auth.user && !auth.setup_required && location.pathname === "/register") return <Register />;
  if (!auth.user) {
    const path = auth.setup_required ? "/first-login" : "/login";
    const params = new URLSearchParams();
    if (next) params.set("next", next);
    if (!auth.setup_required && providerErrorMessage(location.search)) {
      params.set("access_error", new URLSearchParams(location.search).get("access_error")!);
    }
    if (!auth.setup_required && new URLSearchParams(location.search).get("registration") === "pending") params.set("registration", "pending");
    const search = params.size ? `?${params}` : "";
    // Canonicalize entry and drop untrusted continuation parameters.
    if (location.pathname !== path || location.search !== search) return <Navigate to={path + search} replace />;
    return auth.setup_required ? <FirstLogin /> : <Login />;
  }
  if(next)return <ContinueAuthorization to={next}/>;
  if (["/login", "/first-login", "/register"].includes(location.pathname)) return <Navigate to="/" replace />;
  return (
    <Routes>
      <Route path="access/authorize" element={<Authorize/>}/>
      <Route element={<Layout />}>
        <Route path="connections" element={<PageBody><Connections isAdmin={auth.user.role === "admin"}/></PageBody>}/>
        <Route index element={<Home />} />
        <Route path="s/:siteId" element={<SiteScope />}>
          <Route index element={<Navigate to="overview" replace />} />
          <Route path="overview" element={<OverviewPage />} />
          <Route path="people" element={<People />} />
          <Route path="people/:personId" element={<PersonPage />} />
          <Route path="pages" element={<Pages />} />
          <Route path="labels" element={<Labels />} />
          <Route path="segments" element={<Segments />} />
          <Route path="segments/:segmentId" element={<SegmentEditor />} />
          <Route path="install" element={<Install />} />
        </Route>
        <Route path="sites" element={<Sites />} />
        <Route path="settings" element={<Settings />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
