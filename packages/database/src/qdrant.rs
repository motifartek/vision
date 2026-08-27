use qdrant_client::Qdrant;

pub async fn connect(url: &str, api_key: Option<&str>) -> Result<Qdrant, Box<dyn std::error::Error>> {
    let mut builder = Qdrant::from_url(url);
    
    if let Some(key) = api_key {
        builder = builder.api_key(key);
    }
    
    let client = builder.build()?;
    Ok(client)
}