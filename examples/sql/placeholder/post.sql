{{ sink(name="postgres_sink") }}


select * from {{ use_source("json_place_holder") }};
