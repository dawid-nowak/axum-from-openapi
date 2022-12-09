use std::env;
use axum_openapi::generate;
fn main(){
    let out_dir = env::var("OUT_DIR").unwrap();
    let msg = format!("Generating...{out_dir}");
    //println!("cargo:warning={msg}");
    println!("{msg}");
    let api = include_str!("apis/petstore.json");   
    generate(api);
}
