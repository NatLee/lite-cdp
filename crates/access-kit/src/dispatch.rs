use crate::{
    audit,
    types::{AuthContext, Transport},
    AccessError, AccessKit, Invocation,
};
use axum::http::StatusCode;
use serde_json::Value;
use std::time::Instant;
pub(crate) async fn dispatch(
    kit: &AccessKit,
    auth: AuthContext,
    transport: Transport,
    name: &str,
    args: Value,
) -> Result<Value, AccessError> {
    dispatch_result(kit, auth, transport, name, Ok(args)).await
}
pub(crate) async fn dispatch_result(
    kit: &AccessKit,
    auth: AuthContext,
    transport: Transport,
    name: &str,
    args: Result<Value, AccessError>,
) -> Result<Value, AccessError> {
    let registered = kit.0.operations.get(name);
    let safe_name = registered.map(|r| r.operation.name).unwrap_or("unknown");
    let (id, started) = if let Some(ticket) = &auth.audit_ticket {
        (ticket.id.clone(), ticket.started)
    } else {
        (
            audit::reserve(&kit.0.db, &auth, transport, safe_name).await?,
            Instant::now(),
        )
    };
    let result = async {
        let args = args?;
        let op = registered.ok_or_else(|| {
            AccessError::new(
                StatusCode::NOT_FOUND,
                "unknown_operation",
                "Unknown operation",
            )
        })?;
        let principal = kit
            .0
            .host
            .active_principal(auth.principal.user_id)
            .await?
            .ok_or_else(AccessError::unauthorized)?;
        if !auth
            .scopes
            .iter()
            .any(|scope| scope == op.operation.required_scope)
        {
            return Err(AccessError::new(
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "Required scope is missing",
            ));
        }
        if !op.validator.is_valid(&args) || depth(&args) > 32 {
            return Err(AccessError::bad(
                "invalid_arguments",
                "Arguments do not match the operation schema",
            ));
        }
        let invocation = Invocation {
            principal,
            operation: name.to_owned(),
            arguments: args,
        };
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            kit.0.host.execute(invocation),
        )
        .await
        .map_err(|_| AccessError::unavailable())?
    }
    .await;
    let status = result
        .as_ref()
        .map(|_| 200)
        .unwrap_or_else(|e| e.status().as_u16());
    audit::finish(
        &kit.0.db,
        &id,
        status,
        started.elapsed().as_millis().min(i64::MAX as u128) as i64,
        false,
    )
    .await?;
    result
}
fn depth(v: &Value) -> usize {
    match v {
        Value::Object(o) => 1 + o.values().map(depth).max().unwrap_or(0),
        Value::Array(a) => 1 + a.iter().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}
