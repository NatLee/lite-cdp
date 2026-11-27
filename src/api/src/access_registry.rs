//! Fixed read-only domain routing for API/MCP invocations.
//!
//! This module only translates known domain arguments into existing handler
//! queries. Authentication, scopes, consent, tokens and audit belong to access-kit.
use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug)]
pub enum FieldType {
    String,
    Integer,
}
#[derive(Clone, Debug)]
pub struct Field {
    pub name: &'static str,
    pub kind: FieldType,
    pub required: bool,
}
#[derive(Clone, Debug)]
pub struct Operation {
    pub name: &'static str,
    pub path: &'static str,
    pub fields: Vec<Field>,
    pub description: &'static str,
    pub scope: &'static str,
}
#[derive(Debug, PartialEq)]
pub struct RegistryError(pub &'static str);
impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for RegistryError {}
impl Operation {
    pub fn schema(&self) -> Value {
        let properties: Map<String, Value> = self
            .fields
            .iter()
            .map(|f| {
                let rule = match f.kind {
                    FieldType::String => json!({"type":"string","maxLength":4096}),
                    FieldType::Integer => json!({"type":"integer"}),
                };
                (f.name.to_owned(), rule)
            })
            .collect();
        let required: Vec<_> = self
            .fields
            .iter()
            .filter(|f| f.required)
            .map(|f| f.name)
            .collect();
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
    }
}
fn op(
    name: &'static str,
    path: &'static str,
    fields: &'static str,
    required: &'static str,
    description: &'static str,
    scope: &'static str,
) -> Operation {
    let fields = fields
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|name| Field {
            name,
            kind: if matches!(
                name,
                "id" | "site_id"
                    | "person_id"
                    | "tz_offset"
                    | "page"
                    | "per_page"
                    | "days"
                    | "before"
                    | "limit"
            ) {
                FieldType::Integer
            } else {
                FieldType::String
            },
            required: required.split(',').any(|s| s == name),
        })
        .collect();
    Operation {
        name,
        path,
        fields,
        description,
        scope,
    }
}
pub fn operations() -> Vec<Operation> {
    vec![
        op(
            "list_sites",
            "/api/sites",
            "",
            "",
            "Read visible sites without tracker keys or settings",
            "reports:read",
        ),
        op(
            "get_site_overview",
            "/api/sites/{site_id}/overview",
            "site_id,tz_offset",
            "site_id",
            "Read aggregate engagement without identified visitor profiles",
            "reports:read",
        ),
        op(
            "list_pages",
            "/api/sites/{site_id}/pages",
            "site_id,days,q",
            "site_id",
            "Read page engagement without full URLs",
            "reports:read",
        ),
        op(
            "list_segments",
            "/api/sites/{site_id}/segments",
            "site_id",
            "site_id",
            "Read segment names and counts without saved personal criteria",
            "reports:read",
        ),
        op(
            "get_person_events",
            "/api/sites/{site_id}/people/{person_id}/events",
            "site_id,person_id,before,limit,type",
            "site_id,person_id",
            "Read authorized person event details; requires separate people scope",
            "people:read",
        ),
    ]
}

/// Reject extra/nonprimitive input before constructing an internal URI.
pub fn request_path(name: &str, args: &Value) -> Result<String, RegistryError> {
    let operation = operations()
        .into_iter()
        .find(|op| op.name == name)
        .ok_or(RegistryError("unknown domain operation"))?;
    let args = args
        .as_object()
        .ok_or(RegistryError("arguments must be an object"))?;
    if args
        .keys()
        .any(|key| !operation.fields.iter().any(|f| f.name == key))
    {
        return Err(RegistryError("unknown domain argument"));
    }
    let mut values = Vec::new();
    let mut path = operation.path.to_owned();
    for field in &operation.fields {
        let Some(value) = args.get(field.name) else {
            if field.required {
                return Err(RegistryError("missing required domain argument"));
            }
            continue;
        };
        let text = match field.kind {
            FieldType::String => {
                let text = value
                    .as_str()
                    .ok_or(RegistryError("argument must be a string"))?;
                if text.chars().count() > 4096 {
                    return Err(RegistryError("domain argument exceeds length limit"));
                }
                text.to_owned()
            }
            FieldType::Integer => value
                .as_i64()
                .ok_or(RegistryError("argument must be an integer"))?
                .to_string(),
        };
        let placeholder = format!("{{{}}}", field.name);
        if path.contains(&placeholder) {
            if value.as_i64().is_none_or(|id| id <= 0) {
                return Err(RegistryError("identifier must be positive"));
            }
            path = path.replace(&placeholder, &text);
        } else {
            values.push((field.name, text));
        }
    }
    let query = serde_urlencoded::to_string(values)
        .map_err(|_| RegistryError("cannot encode domain arguments"))?;
    Ok(if query.is_empty() {
        path
    } else {
        format!("{path}?{query}")
    })
}

fn keep(value: Value, fields: &[&str]) -> Result<Value, RegistryError> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or(RegistryError("expected object response"))?;
    object.retain(|key, _| fields.contains(&key.as_str()));
    Ok(Value::Object(object))
}
fn rows(value: Value, fields: &[&str]) -> Result<Value, RegistryError> {
    let values = value
        .as_array()
        .ok_or(RegistryError("expected array response"))?;
    values
        .iter()
        .cloned()
        .map(|value| keep(value, fields))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}
/// Allowlisted views never expose collection keys, delivery targets or hidden profile fields.
pub fn project(operation: &str, value: Value) -> Result<Value, RegistryError> {
    match operation {
        "list_sites" => rows(
            value,
            &[
                "id",
                "name",
                "url",
                "active",
                "created_at",
                "total",
                "unhandled",
                "people",
                "identified",
                "events",
                "last_event_at",
            ],
        ),
        "get_site_overview" => keep(
            value,
            &[
                "engagement",
                "totals",
                "last_7_days",
                "previous_7_days",
                "daily",
                "top_pages",
                "top_referrers",
                "labels",
            ],
        ),
        "list_pages" => rows(
            value,
            &[
                "path",
                "title",
                "pageviews",
                "visitors",
                "last_at",
                "labels",
            ],
        ),
        "list_segments" => rows(
            value,
            &[
                "id",
                "name",
                "description",
                "kind",
                "count",
                "created_at",
                "updated_at",
            ],
        ),
        "get_person_events" => Ok(value),
        _ => Err(RegistryError("unknown projection")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_are_fixed_and_arguments_encoded() {
        assert!(request_path("list_sites", &json!({"url":"/api/users"})).is_err());
        assert!(request_path("list_sites", &json!([])).is_err());
        let encoded = request_path(
            "list_pages",
            &json!({"site_id":1,"q":"tea&user_id=1#fragment"}),
        )
        .unwrap();
        assert!(encoded.contains("%26user_id%3D1%23fragment") && !encoded.contains("&user_id="));
        assert!(request_path("list_pages", &json!({"q":{"url":"/api/users"}})).is_err());
        for op in operations() {
            assert_eq!(op.schema()["additionalProperties"], false);
            assert!(op.path.starts_with("/api/"));
        }
    }
    #[test]
    fn site_secrets_are_removed() {
        let projected=project("list_sites",json!([{"id":1,"name":"safe","key":"SECRET","notify_email":"PRIVATE","webhook_url":"PRIVATE","members":[1]}])).unwrap();
        assert_eq!(projected, json!([{"id":1,"name":"safe"}]));
        assert!(project("list_sites", json!({})).is_err());
        assert_eq!(project("list_segments",json!([{"id":1,"name":"safe","rules":{"email":"PERSONAL-CANARY"},"created_by_name":"PRIVATE"}])).unwrap(),json!([{"id":1,"name":"safe"}]));
        assert_eq!(
            project(
                "list_pages",
                json!([{"path":"/safe","url":"https://example.invalid/?token=SECRET"}])
            )
            .unwrap(),
            json!([{"path":"/safe"}])
        );
    }
}
