import { Badge } from "../components/ui";
export function AccountRegistrationStatus({ status }: { status?: "active" | "pending" | "rejected" }) {
  return status === "pending" ? <span className="ml-2"><Badge>待審核・尚未開通</Badge></span> : status === "rejected" ? <span className="ml-2"><Badge tone="red">申請已拒絕</Badge></span> : null;
}
