use multimap::MultiMap;
use openapiv3::{Components, OpenAPI, Operation, ReferenceOr, RequestBody, Schema};
use proc_macro2::TokenStream;
use quote::quote;
use serde_json;
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use quote::format_ident;
    
fn schema_reference_resolver(
    reference: ReferenceOr<Schema>,
    components: Components,
) -> Option<Schema> {
    match reference {
        ReferenceOr::Item(item) => Some(item),
        ReferenceOr::Reference { reference } => {
            let mut parts: Vec<&str> = reference.split('/').collect();
            let maybe_name = parts.pop();
            let maybe_component_name = parts.pop();

            if let (Some(name), Some(path)) = (maybe_name, maybe_component_name) {
                match path {
                    "schemas" => {
                        if let Some(comp) = components.schemas.get(name) {
                            schema_reference_resolver(comp.clone(), components)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
    }
}

fn request_bodies_reference_resolver(
    reference: ReferenceOr<RequestBody>,
    components: Components,
) -> Option<RequestBody> {
    match reference {
        ReferenceOr::Item(item) => Some(item),
        ReferenceOr::Reference { reference } => {
            let mut parts: Vec<&str> = reference.split('/').collect();
            let maybe_name = parts.pop();
            let maybe_component_name = parts.pop();

            if let (Some(name), Some(path)) = (maybe_name, maybe_component_name) {
                match path {
                    "request_bodies" => {
                        if let Some(comp) = components.request_bodies.get(name) {
                            request_bodies_reference_resolver(comp.clone(), components)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
    }
}

#[derive(Debug)]
struct HandlerDescription {
    operation_id: String,
    method: http::method::Method,
    path: String,
}

fn process_operation(
    method: http::method::Method,
    path: String,
    handler_descriptions: &mut MultiMap<String, HandlerDescription>,
    operation: &Operation,
) {
    if let Some(operation_id) = &operation.operation_id {
        println!("Path {path:?} {method} operation {operation_id:?}");
        let hd = HandlerDescription {
            operation_id: operation_id.clone(),
            method,
            path,
        };
        let tag = if let Some(tag) = operation.tags.first() {
            tag.clone()
        } else {
            "NoTag".to_string()
        };
        handler_descriptions.insert(tag, hd);
        let responses = &operation.responses;
    } else {
        println!("Path {path:?} no operation id");
    }

    println!("*********************");
}



fn generate_router(name: &String, handler_descriptions: &Vec<HandlerDescription>) -> TokenStream {
    use convert_case::{Case, Casing};
    let mut handlers = vec![];
    let mut paths = vec![];
    let mut methods = vec![];
    let mut handler_names = vec![];
    for hd in handler_descriptions {
	let method_name = hd.operation_id.to_case(Case::Snake);
	let method_name_ident = format_ident!("{}", method_name);
        handlers.push(quote! {
	    pub async fn #method_name_ident()-> impl IntoResponse {
		Json(json!({"message":"generated with love"}))
	    }
	    
        }
        );
	
        paths.push(&hd.path);
	methods.push(format_ident!("{}",hd.method.to_string().to_ascii_lowercase()));
	handler_names.push(format_ident!("{}",method_name));
	
    }

    

    quote! {
    use axum::{routing::{get, post}, Router, response::IntoResponse, Json};
        use serde_json::{json, Value};
        #(#handlers)*

	pub fn router() -> Router{
            Router::new()	    
 		#(.route(#paths,#methods(#handler_names)))*
	}
    }
}

pub fn save_generated_file(file_name: &str, content: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join(&file_name);
    let mut f = BufWriter::new(File::create(&dest_path).unwrap());
    write!(f, "{}", &content).unwrap();
}

pub fn generate(name: &str) {
    //let data = include_str!(name);
    let openapi: OpenAPI = serde_json::from_str(name).expect("Could not deserialize input");
    let paths = openapi.paths;

    let mut handler_descriptions = MultiMap::<String, HandlerDescription>::new();

    for (path_name, path_item) in paths.iter() {
        println!("Path {path_name:?} {path_item:?}");
        println!("------------------\n\n");
        match path_item {
            ReferenceOr::Item(item) => {
                if let Some(operation) = &item.get {
                    process_operation(
                        http::method::Method::GET,
                        path_name.clone(),
                        &mut handler_descriptions,
                        operation,
                    );
                }
                if let Some(operation) = &item.post {
                    process_operation(
                        http::method::Method::POST,
                        path_name.clone(),
                        &mut handler_descriptions,
                        operation,
                    );
                }
                if let Some(operation) = &item.put {
                    process_operation(
                        http::method::Method::PUT,
                        path_name.clone(),
                        &mut handler_descriptions,
                        operation,
                    );
                }
            }
            _ => {
                println!("Path {path_name:?} {path_item:?}");
            }
        }
    }

    for (name, desc) in handler_descriptions.iter_all() {
        println!("{name}, {desc:?}");
        let router = generate_router(name, desc);
        println!("Router  {router}");
	save_generated_file(&format!("{name}_router.rs.src"), &router.to_string());
	let ast  = syn::parse2(router).unwrap();	
        let code = prettyplease::unparse(&ast);
	
        save_generated_file(&format!("{name}_router.rs"), &code);	
    }

    // println!("\n\n{:?}\n\n", openapi.components);

    // if let Some(components)  = openapi.components{
    // 	for (key,value) in components.schemas.iter(){
    // 	    println!("Schema {key:?} {value:?}");
    // 	}
    // 	for (key,value) in components.request_bodies.iter(){
    // 	    println!("Body {key:?} {value:?}");
    // 	}

    // 	for (key,value) in components.responses.iter(){
    // 	    println!("Response {key:?} {value:?}");
    // 	}
    // }

    //    println!("{:?}", openapi);
}
