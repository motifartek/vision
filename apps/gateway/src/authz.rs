use tonic::transport::Channel;
use crate::error::GatewayError;

pub mod keto {
    tonic::include_proto!("ory.keto.relation_tuples.v1alpha2");
}

use keto::check_service_client::CheckServiceClient;

#[derive(Clone)]
pub struct AuthzState {
    pub client: CheckServiceClient<Channel>,
}

pub async fn check_permission(
    state: &AuthzState,
    subject_id: &str,
    namespace: &str,
    object: &str,
    relation: &str,
) -> Result<bool, GatewayError> {
    let mut client = state.client.clone();

    let request = tonic::Request::new(keto::CheckRequest {
        tuple: Some(keto::RelationTuple {
            namespace: namespace.to_string(),
            object: object.to_string(),
            relation: relation.to_string(),
            subject: Some(keto::Subject {
                r#ref: Some(keto::subject::Ref::Id(subject_id.to_string())),
            }),
        }),
        latest: true,
        snaptoken: String::new(),
    });

    let response = client.check(request).await?;
    let allowed = response.into_inner().allowed;

    Ok(allowed)
}
