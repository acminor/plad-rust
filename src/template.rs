use serde_derive::Deserialize;
use std::fs;
use std::io::Read;

use num::Complex;

use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;

#[derive(Debug, Deserialize)]
pub struct TemplateToml {
    pub templates: String,
    pub pre_fft: bool,
}

pub struct TemplateGroup {
}

//#[derive(Debug)]
pub struct Templates {
    //pub templates: Vec<Vec<Complex<f32>>>,
    pub templates: AF_Array<Complex<f32>>,
    //pub widths: Vec<usize>,
    pub pre_fft: bool,
}

pub fn parse_template_file(file_name: String) -> Templates {
    let contents = {
        let contents = fs::read_to_string(file_name)
            .expect("Failed to read Templates TOML file");

        contents
    };

    let template_toml: TemplateToml =
        toml::from_str(&contents).expect("Failed to parse Templates TOML file");

    let templates: Vec<Vec<Complex<f32>>> = {
        let mut file = fs::File::open(&template_toml.templates)
            .expect("Failed to read Templates templates file");
        let mut contents: Vec<u8> = Vec::new();
        file.read_to_end(&mut contents)
            .expect("Failed reading contents of templates.");

        let mut de = rmp_serde::Deserializer::new(&contents[..]);

        let temp: Vec<Vec<(f32, f32)>> =
            serde::Deserialize::deserialize(&mut de)
                .expect("Failed to deserialize templates");

        let max_temp_len = temp
            .iter()
            .map(|template| template.len())
            .max()
            .expect("Error finding max template length.");

        temp.into_iter()
            .map(|array| {
                array
                    .iter()
                    .map(|&(x, y)| Complex::new(x, y))
                    .chain(
                        (0..max_temp_len)
                            .map(|x| Complex::new(0.0 as f32, 0.0 as f32)),
                    )
                    .collect()
            })
            .collect()
    };

    let width = templates[0].len();
    println!("width {}", width);
    let templates = &templates
        .iter()
        .flat_map(|signal| signal.into_iter())
        .cloned()
        .collect::<Vec<Complex<f32>>>()[..];
    let templates = AF_Array::new(
        templates,
        AF_Dim4::new(&[(templates.len() / width) as u64, width as u64, 1, 1]),
    );

    //println!("is sparse {}", templates.is_sparse());
    let templates = AF::sparse_from_dense(&templates, AF::SparseFormat::CSC);

    //let templates = Templates{
    //        pre_fft: template_toml.pre_fft
    //};

    Templates {
        templates: templates,
        pre_fft: true,
    }
}
