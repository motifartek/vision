use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

pub async fn connect(url: &str, ns: &str, db: &str) -> Result<Surreal<Client>, Box<dyn std::error::Error>> {
    let client = Surreal::new::<Ws>(url).await?;
    
    // Varsayilan root girisi (gelistirme ortami icin)
    client
        .signin(Root {
            username: "root",
            password: "root",
        })
        .await?;
        
    client.use_ns(ns).use_db(db).await?;
    
    Ok(client)
}