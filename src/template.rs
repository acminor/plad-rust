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
    pub templates: AF_Array<Complex<f32>>,
    pub max_len: usize,
}

pub struct Templates {
    pub templates: Vec<TemplateGroup>,
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

    let templates: Vec<TemplateGroup> = {
        let mut file = fs::File::open(&template_toml.templates)
            .expect("Failed to read Templates templates file");
        let mut contents: Vec<u8> = Vec::new();
        file.read_to_end(&mut contents)
            .expect("Failed reading contents of templates.");

        let mut de = rmp_serde::Deserializer::new(&contents[..]);

        let temp: Vec<Vec<(f32, f32)>> =
            serde::Deserialize::deserialize(&mut de)
                .expect("Failed to deserialize templates");

        let temp = temp
            .into_iter()
            .map(|template| {
                template
                    .into_iter()
                    .map(|(x, y)| Complex::new(x, y))
                    .collect()
            })
            .collect::<Vec<Vec<Complex<f32>>>>();

        let max_len = temp
            .iter()
            .map(|template| template.len())
            .max()
            .expect("Issue getting max template set length.");

        temp.chunks(2560)
            .map(|chunk| {
                /*
                let max_len = chunk
                    .iter()
                    .map(|template| template.len())
                    .max()
                    .expect("Issue getting max template set length.");
                */
                let chunk_len = chunk.len();
                let padded_temps = chunk
                    .into_iter()
                    .map(|template| {
                        let temp_len = template.len();
                        template
                            .into_iter()
                            .chain(
                                (temp_len..max_len)
                                    .map(|x| {
                                        Complex::new(0.0 as f32, 0.0 as f32)
                                    })
                                    .collect::<Vec<Complex<f32>>>()
                                    .iter(),
                            )
                            .cloned()
                            .collect::<Vec<Complex<f32>>>()
                    })
                    .flat_map(|template| template)
                    .collect::<Vec<Complex<f32>>>();

                /*
                println!(
                    "Len: {}, ExLen: {}",
                    padded_temps.len(),
                    max_len * chunk_len
                );
                */

                let chunk = AF_Array::new(
                    &padded_temps,
                    AF_Dim4::new(&[chunk_len as u64, max_len as u64, 1, 1]),
                );

                let chunk = AF::transpose(&chunk, false);

                TemplateGroup {
                    templates: chunk,
                    max_len: max_len,
                }
            })
            .collect::<Vec<TemplateGroup>>()
    };

    Templates {
        templates: templates,
        pre_fft: true,
    }
}
