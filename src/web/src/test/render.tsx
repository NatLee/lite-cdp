import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { App } from "../App";
import { ToastProvider } from "../components/ui";
import { AppProvider } from "../context";

/** 把整個 App 畫出來，從 `path` 這個網址開始。 */
export function renderApp(path = "/") {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppProvider>
        <ToastProvider>
          <App />
        </ToastProvider>
      </AppProvider>
    </MemoryRouter>,
  );
}
