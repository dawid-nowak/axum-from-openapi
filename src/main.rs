use axum_openapi::generate;

//mod temp;
pub fn main(){
    let api = include_str!("../petstore.json");
    std::env::set_var("OUT_DIR","./target");
    generate(api);
}
