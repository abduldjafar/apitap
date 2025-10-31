{{ sink(name="postgres_sink") }}


select 
    "userId" as id,
    count(1) as cnt
from {{ use_source("json_place_holder2") }}
group by 1
order by 2
