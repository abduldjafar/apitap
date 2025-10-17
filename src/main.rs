use apitap::errors::Result;
use datafusion::common::HashMap;
use reqwest::RequestBuilder;

pub enum PaginationType{
    Paging,
    Offset
}

pub struct HttpRequest{
    url : String,
    params: Option<HashMap<String,String>>,
    header: Option<HashMap<String,String>>,
    bearer_auth: Option<String>,
    pagination_type: Option<PaginationType>,
    data_field:Option<String>
}

impl HttpRequest {
    pub fn new(url: impl Into<String>) -> Self {
        let url = url.into();
        Self {
            url,
            params: None,
            header: None,
            bearer_auth: None,
            pagination_type:None,
            data_field:None
        }
    }
    pub fn param(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let map = self.params.get_or_insert_with(HashMap::new);
        map.insert(key.into(), value.into());
        self
    }

    pub fn header(&mut self, key:  impl Into<String>, value:  impl Into<String>) -> &mut Self {
        let map = self.header.get_or_insert_with(HashMap::new);
        map.insert(key.into(), value.into());
        self
    }

    pub fn bearer_auth(&mut self,token: impl Into<String>) -> &mut Self {
        self.bearer_auth = Some(token.into());
        self
    }

    pub fn data_field(&mut self,field:impl Into<String>) -> &mut Self{
        self.data_field = Some(field.into());
        self
    }

    pub fn pagination_type(&mut self, pagination_type:PaginationType) -> &mut Self {
        self.pagination_type = Some(pagination_type);
        self
    }


    pub fn build(&mut self) {
        let client = reqwest::Client::new();
        let mut req = client.get(&self.url);

       if let Some(params) = &self.params {
           for (key, value) in params {
                req = req.query(&[(key, value)]);
            }
       }

       if let Some(headers) = &self.header {
            for (key,value) in headers {
                req = req.header(key, value)
            }
       }

       if let Some(pagination_type) = &self.pagination_type {
                match pagination_type {
                    PaginationType::Paging => todo!(),
                    PaginationType::Offset => todo!(),
                }
       }


    }
}


#[tokio::main]
async fn main() -> Result<()> {
    let new_req = HttpRequest::new( "asoiurl" )
                                .param("key", "value")
                                .param("key2", "value")
                                .bearer_auth("asoi")
                                .build();
    Ok(())
}



