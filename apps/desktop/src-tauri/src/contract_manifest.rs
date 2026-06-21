use serde::de::DeserializeOwned;

pub(crate) fn parse_contract_json<T>(json: &'static str, manifest_name: &str) -> T
where
    T: DeserializeOwned,
{
    serde_json::from_str(json)
        .unwrap_or_else(|err| panic!("{manifest_name} must be valid JSON: {err}"))
}
