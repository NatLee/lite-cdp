import { AuthScreen } from "../components/AuthScreen";
import { RegisterForm } from "../registration/RegisterForm";
export function Register() { return <AuthScreen setupRequired={false}><RegisterForm /></AuthScreen>; }
