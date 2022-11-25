use serde_json;
use openapiv3::{OpenAPI,ReferenceOr,Schema,Components,RequestBody};


fn schema_reference_resolver(reference: ReferenceOr<Schema>, components: Components)->Option<Schema>{
    match reference{
	ReferenceOr::Item(item) => Some(item),
	ReferenceOr::Reference{
	    reference}
	=> {
	    let mut parts: Vec<&str> = reference.split('/').collect();
	    let maybe_name = parts.pop();
	    let maybe_component_name = parts.pop();
	    
	    if let (Some(name),Some(path)) = (maybe_name, maybe_component_name){
		match path{
		    "schemas" => {
			if let Some(comp) = components.schemas.get(name){			    
			    schema_reference_resolver(comp.clone(), components)
			}else{
			    None
			}
		    },
		    _ => None
		}
	    }else{
		None
	    }	    			    	    
	}
    }
}

fn request_bodies_reference_resolver(reference: ReferenceOr<RequestBody>, components: Components)->Option<RequestBody>{
    match reference{
	ReferenceOr::Item(item) => Some(item),
	ReferenceOr::Reference{
	    reference}
	=> {
	    let mut parts: Vec<&str> = reference.split('/').collect();
	    let maybe_name = parts.pop();
	    let maybe_component_name = parts.pop();
	    
	    if let (Some(name),Some(path)) = (maybe_name, maybe_component_name){
		match path{
		    "request_bodies" => {
			if let Some(comp) = components.schemas.get(name){			    
			    request_bodies_reference_resolver(comp.clone(), components)
			}else{
			    None
			}
		    },
		    _ => None
		}
	    }else{
		None
	    }	    			    	    
	}
    }
}


fn main() {
    let data = include_str!("petstore.json");
    let openapi: OpenAPI = serde_json::from_str(data).expect("Could not deserialize input");
    let paths = openapi.paths;
    
    for (path_name, path_item) in  paths.iter(){
	match path_item{
	    ReferenceOr::Item(item) =>{		
		if let Some(operation) = &item.get{
		    let id = &operation.operation_id;
		    let responses = &operation.responses;		    
		    println!("Path {path_name:?} GET operation {id:?} {responses:?}");
		    println!("*********************");
		}
		
	    }
	    _ => {
		println!("Path {path_name:?} {path_item:?}");
	    }
	}
	
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
