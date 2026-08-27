//! Keto gRPC istemcisini `proto/keto.proto` dosyasindan uretir.
//!
//! `protoc` sisteme kurulmus olmak zorunda degil: bulunamazsa paketlenmis
//! ikili kullaniliyor. Boylece depoyu klonlayan herkes ek kurulum yapmadan
//! `cargo build` diyebiliyor — sartname "tekrar uretilebilir olmalidir"
//! diyor ve eksik protoc butun workspace derlemesini dusuruyordu.

fn main() -> Result<(), Box<dyn std::error::Error>> {
<<<<<<< HEAD
    std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
=======
    if std::env::var_os("PROTOC").is_none() {
        if let Ok(yol) = protoc_bin_vendored::protoc_bin_path() {
            std::env::set_var("PROTOC", yol);
        }
    }

>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
    tonic_build::compile_protos("proto/keto.proto")?;
    Ok(())
}