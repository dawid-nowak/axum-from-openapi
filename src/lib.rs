use multimap::MultiMap;
use openapiv3::{Components, OpenAPI, Operation, Parameter, ReferenceOr, RequestBody};
use proc_macro2::TokenStream;
use quote::quote;
use serde_json;
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use convert_case::{Case, Casing};
use proc_macro2::Ident;
use quote::format_ident;
use std::sync::Arc;

#[derive(Clone)]
struct ComponentReferenceProvider {
    components: Arc<Components>,
}

struct EmptyReferenceProvider;

trait ReferenceProvider {
    fn resolve_parameter(&self, name: &str, path: &str) -> Option<ReferenceOr<Parameter>>;
    fn resolve_request_body(&self, name: &str, path: &str) -> Option<ReferenceOr<RequestBody>>;
}

impl ReferenceProvider for ComponentReferenceProvider {
    fn resolve_parameter(&self, name: &str, path: &str) -> Option<ReferenceOr<Parameter>> {
        match path {
            "parameters" => self.components.parameters.get(name).cloned(),
            _ => None,
        }
    }

    fn resolve_request_body(&self, name: &str, path: &str) -> Option<ReferenceOr<RequestBody>> {
        match path {
            "request_body" => self.components.request_bodies.get(name).cloned(),
            _ => None,
        }
    }
}

impl ReferenceProvider for EmptyReferenceProvider {
    fn resolve_parameter(&self, _name: &str, _path: &str) -> Option<ReferenceOr<Parameter>> {
        None
    }
    fn resolve_request_body(&self, _name: &str, _path: &str) -> Option<ReferenceOr<RequestBody>> {
        None
    }
}

fn reference_resolver<T, F>(request_body: ReferenceOr<T>, reference_provider: F) -> Option<T>
where
    F: Fn(&str, &str) -> Option<ReferenceOr<T>>,
{
    match request_body {
        ReferenceOr::Item(item) => Some(item),
        ReferenceOr::Reference { reference } => {
            let mut parts: Vec<&str> = reference.split('/').collect();
            let maybe_name = parts.pop();
            let maybe_component_name = parts.pop();

            if let (Some(name), Some(path)) = (maybe_name, maybe_component_name) {
                if let Some(new_ref) = reference_provider(name, path) {
                    reference_resolver(new_ref, reference_provider)
                } else {
                    None
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
    path_params: Vec<String>,
    request_body: Option<RequestBody>,
}

fn process_operation(
    method: http::method::Method,
    path: String,
    handler_descriptions: &mut MultiMap<String, HandlerDescription>,
    operation: &Operation,
    reference_provider: &Box<dyn ReferenceProvider>,
) {
    if let Some(operation_id) = &operation.operation_id {
        println!("Path {path:?} {method} operation {operation_id:?}");

        let mut path_params = vec![];

        let ptr = |x: &str, y: &str| reference_provider.resolve_parameter(x, y);

        for parameter in &operation.parameters {
            if let Some(parameter) = reference_resolver(parameter.clone(), ptr) {
                match parameter {
                    Parameter::Path {
                        parameter_data,
                        style: _,
                    } => {
                        path_params.push(parameter_data.name.to_case(Case::Snake));
                    }
                    _ => {}
                }
            }
        }

        let request_body = if let Some(request_body) = &operation.request_body {
            let ptr = |x: &str, y: &str| reference_provider.resolve_request_body(x, y);
            reference_resolver(request_body.clone(), ptr)
        } else {
            None
        };

        let hd = HandlerDescription {
            operation_id: operation_id.to_case(Case::Snake),
            method,
            path: modify_path_template(&path),
            path_params,
            request_body,
        };
        let tag = if let Some(tag) = operation.tags.first() {
            tag.clone()
        } else {
            "NoTag".to_string()
        };
        handler_descriptions.insert(tag, hd);
        let _responses = &operation.responses;
    } else {
        println!("Path {path:?} no operation id");
    }

    println!("*********************");
}

fn adjust_content_type(ct: &str) -> String {
    ct.replace("/", "_")
}

fn generate_content_handlers(
    hd: &HandlerDescription,
) -> (Vec<((String, Ident), Ident)>, TokenStream) {
    let http_method_ident = format_ident!("{}", hd.method.to_string().to_ascii_lowercase());
    let method_name = hd.operation_id.to_case(Case::Snake);
    let mut handler_names = vec![];
    let mut handler = quote! {};
    let mut param_idents: Vec<Ident> = vec![];

    let params = if hd.path_params.is_empty() {
        quote! {}
    } else {
        param_idents = hd
            .path_params
            .iter()
            .map(|p| format_ident!("{}", p))
            .collect();
        quote! {
            #(Path(#param_idents): Path<String>),*
        }
    };

    if let Some(request_body) = &hd.request_body {
        for (content_type, _media_type) in &request_body.content {
            let ct = adjust_content_type(&content_type);
            let method_name_ident = format_ident!("{}_{}", method_name, ct);
            let method_name_prefix = quote! {
                pub async fn #method_name_ident
            };

            let body_type_ident = format_ident!("{}", "Value");
            let body_param = if request_body.required {
                quote! {
                    Json(body): Json<#body_type_ident>
                }
            } else {
                quote! {
                    body: Option<Json<#body_type_ident>>
                }
            };

            let params = params.clone();

            let method_signature = if hd.path_params.is_empty() {
                quote! {
                    #method_name_prefix(#body_param)-> impl IntoResponse{
                    Json(json!({
                        "message":"generated with love",
                        "body": format!("{:?}",body)
                    }
                    ))
                    }
                }
            } else {
                quote! {
                    #method_name_prefix(#params, #body_param)-> impl IntoResponse{
                    Json(json!({
                        "message":"generated with love",
                        "params": [
                        #(#param_idents),*
                        ],
                        "body": format!("{:?}",body)
                    }
                    ))
                    }
                }
            };
            handler_names.push((
                (hd.path.clone(), http_method_ident.clone()),
                method_name_ident,
            ));
            handler.extend(method_signature);
        }
    } else {
        let method_name_ident = format_ident!("{}", method_name);
        let method_name_prefix = quote! {
            pub async fn #method_name_ident
        };

        let method_signature = quote! {
            #method_name_prefix(#params)-> impl IntoResponse{
                Json(json!({
                "message":"generated with love",
                "params": [
                    #(#param_idents),*
                ]
                }
                ))
            }
        };
        handler_names.push(((hd.path.clone(), http_method_ident), method_name_ident));
        handler.extend(method_signature);
    };

    (handler_names, handler)
}
fn generate_handlers(
    handler_descriptions: &Vec<HandlerDescription>,
) -> (Vec<((String, Ident), Ident)>, TokenStream) {
    let mut handlers = vec![];
    let mut handler_names = vec![];
    for hd in handler_descriptions {
        let (content_handler_names, tokens) = generate_content_handlers(hd);
        handlers.push(tokens);
        handler_names.extend(content_handler_names);
    }
    (
        handler_names,
        quote! {
        use axum::{response::IntoResponse};
        use axum::extract::{Path,Json};
            use serde_json::{json, Value};
            #(#handlers)*
        },
    )
}
fn modify_path_template(path: &str) -> String {
    if let Some((l, r)) = path.split_once("{") {
        let modified = if let Some((l, r)) = r.split_once("}") {
            let mut l = format!(":{}", l.to_case(Case::Snake));
            l.push_str(r);
            l
        } else {
            r.to_string()
        };
        let mut l = l.to_string();
        l.push_str(&modified);
        modify_path_template(&l)
    } else {
        path.to_string()
    }
}

fn generate_router(name: &str, names: Vec<((String, Ident), Ident)>) -> TokenStream {
    let (paths_methods, handler_names): (Vec<(String, Ident)>, Vec<Ident>) =
        names.into_iter().unzip();
    let (paths, methods): (Vec<String>, Vec<Ident>) = paths_methods.into_iter().unzip();

    let handlers_mod_name = format_ident!("{}_handlers", name);
    quote! {
        mod pets{
        use axum::{routing::{get, post,put}, Router, response::IntoResponse, Json};

        use crate::handlers::#handlers_mod_name;

        pub fn router() -> Router{
                Router::new()
            #(.route(#paths,#methods(#handlers_mod_name::#handler_names)))*
        }
        }
    }
}

pub fn save_generated_file(file_name: &str, content: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    save_generated_file_to_dir(&out_dir, file_name, content);
}

pub fn save_generated_file_to_dir(dir: &str, file_name: &str, content: &str) {
    let dest_path = Path::new(&dir).join(&file_name);
    let mut f = BufWriter::new(File::create(&dest_path).unwrap());
    write!(f, "{}", &content).unwrap();
}

pub fn sanitize_and_save(name: &str, tokens: TokenStream) {
    let out_dir = env::var("OUT_DIR").unwrap();
    sanitize_and_save_to_dir(name, &out_dir, tokens)
}
pub fn sanitize_and_save_to_dir(name: &str, dir: &str, tokens: TokenStream) {
    save_generated_file_to_dir(dir, &format!("{name}.rs.src"), &tokens.to_string());
    let ast = syn::parse2(tokens).unwrap();
    let code = prettyplease::unparse(&ast);
    save_generated_file_to_dir(dir, &format!("{name}.rs"), &code);
}

pub fn generate_server(prefixes: Vec<String>, routers: Vec<Ident>) -> TokenStream {
    let mod_names: Vec<String> = routers.iter().map(|k| format!("/{}.rs", k)).collect();
    quote!(
        use axum::Router;
        #(include!(concat!(env!("OUT_DIR"), #mod_names)))*;

        #[allow(dead_code)]
        pub fn server() -> Router {

            Router::new()
            #(.nest(#prefixes, #routers::router()))*
        }
    )
}

pub fn generate(name: &str) {
    let openapi: OpenAPI = serde_json::from_str(name).expect("Could not deserialize input");
    let paths = openapi.paths;
    let reference_provider: Box<dyn ReferenceProvider> = match openapi.components {
        Some(components) => Box::new(ComponentReferenceProvider {
            components: Arc::new(components),
        }),
        None => Box::new(EmptyReferenceProvider),
    };

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
                        &reference_provider,
                    );
                }
                if let Some(operation) = &item.post {
                    process_operation(
                        http::method::Method::POST,
                        path_name.clone(),
                        &mut handler_descriptions,
                        operation,
                        &reference_provider,
                    );
                }
                if let Some(operation) = &item.put {
                    process_operation(
                        http::method::Method::PUT,
                        path_name.clone(),
                        &mut handler_descriptions,
                        operation,
                        &reference_provider,
                    );
                }
            }
            _ => {
                println!("Path {path_name:?} {path_item:?}");
            }
        }
    }

    if !handler_descriptions.is_empty() {
        if std::path::Path::new("./src/handlers").try_exists().is_err() {
            std::fs::create_dir("./src/handlers").unwrap();
        }
    }

    let mut routers_ident = vec![];
    let mut routes_prefixes = vec![];

    for (name, desc) in handler_descriptions.iter_all() {
        println!("{name}, {desc:?}");
        let (names, handlers) = generate_handlers(desc);
        println!("Handlers  {handlers}");
        sanitize_and_save_to_dir(&format!("{name}_handlers"), "./src/handlers", handlers);
        println!("Handlers  sanitized");
        let router = generate_router(name, names);
        sanitize_and_save(&format!("{name}"), router);
        routers_ident.push(format_ident!("{name}"));
        routes_prefixes.push(format!("/"));
    }

    let mods: Vec<Ident> = handler_descriptions
        .keys()
        .map(|k| format_ident!("{}_handlers", k))
        .collect();
    if !mods.is_empty() {
        let tokens = quote!(
            #(pub mod #mods;)*
        );
        sanitize_and_save_to_dir("mod", "./src/handlers", tokens);
    }

    let server = generate_server(routes_prefixes, routers_ident);
    sanitize_and_save("lib", server);

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
