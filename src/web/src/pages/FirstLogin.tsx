import { AuthScreen } from "../components/AuthScreen";

/** Available only while the installation has no accounts. */
export function FirstLogin() { return <AuthScreen setupRequired />; }
