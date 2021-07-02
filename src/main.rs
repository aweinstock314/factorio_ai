use nom::{
    error::{convert_error, VerboseError},
    Finish,
};
use std::{error::Error, fs::File, io::Read};
use ron::ser::PrettyConfig;

pub mod lua_parser;

use lua_parser::{parse_data_extend, LuaObject};

#[derive(Debug, Clone)]
struct Recipe {
    name: String,
    enabled: bool,
    ingredients: Vec<(String, usize)>,
    energy_required: usize,
    result: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = File::open("../factorio_headless/factorio/data/base/prototypes/recipe.lua")?;

    let mut string_data = String::new();
    data.read_to_string(&mut string_data)?;

    let recipes: Result<_, VerboseError<_>> = parse_data_extend(&string_data).finish();
    match recipes {
        //Ok((_, obj)) => println!("{}", pretty(&obj, 0)),
        Ok((_, obj)) => println!("{}", ron::ser::to_string_pretty(&obj, PrettyConfig::default())?),
        Err(e) => println!("{}", convert_error(&*string_data, e)),
    }

    Ok(())
}
