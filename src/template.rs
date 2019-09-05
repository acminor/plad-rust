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
    pub num_templates: usize,
    pub max_len: usize,
    pub fft_len: usize,
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

        let temp: Vec<Vec<f32>> =
            serde::Deserialize::deserialize(&mut de)
                .expect("Failed to deserialize templates");

        /*
        let temp = temp
            .into_iter()
            .map(|template| {
                template
                    .into_iter()
                    .map(|(x, y)| Complex::new(x, y))
                    .collect()
            })
            .collect::<Vec<Vec<Complex<f32>>>>();
        */

        let max_len = temp
            .iter()
            .map(|template| template.len())
            .max()
            .expect("Issue getting max template set length.");

        // using the numpy fftfreq reference
        // [ ] TODO check if correct
        // - ie only concerned with pos. freq. in fft
        let real_len: usize = if max_len % 2 == 1 { // odd
            (max_len-1)/2
        } else { // even
            max_len/2 - 1
        };

        temp.chunks(2800)
            .map(|chunk| {
                let chunk_len = chunk.len();

                let mut chunk: Vec<AF_Array<Complex<f32>>> =
                    chunk.into_iter().map(|template| {
                        let template = AF_Array::new(
                            &template,
                            AF_Dim4::new(&[template.len() as u64, 1, 1, 1])
                        );

                        let fft_bs = AF::fft(&template, 1.0, max_len as i64);
                        let temp = AF::rows(&fft_bs, 0, (real_len - 1) as u64);
                        //temp
                        AF::conjg(&temp)
                    }).collect();

                let mut chunk = chunk.drain(0..chunk.len());
                let chunk_out = {
                    let mut chunk_out = chunk.next()
                        .expect("Should have at least one template.");
                    for lchunk in chunk {
                        chunk_out = AF::join(1, &chunk_out, &lchunk);
                    }

                    chunk_out
                };
                println!("J Dims: {}", chunk_out.dims());
                //let chunk_out = AF::transpose(&chunk_out, false);

                let mut buf: Vec<Complex<f32>> = Vec::new();
                buf.resize(chunk_out.elements(), Complex::new(0.0, 0.0 as f32));
                chunk_out.lock();
                chunk_out.host(&mut buf);
                chunk_out.unlock();

                // [ ] TODO check that plot is correct
                /*
                crate::utils::debug_plt(
                    &buf.iter().map(|x| x.re).collect(), None);
                */

                TemplateGroup {
                    templates: chunk_out,
                    max_len: real_len,
                    fft_len: max_len,
                    num_templates: chunk_len,
                }
            })
            .collect::<Vec<TemplateGroup>>()
    };

    Templates {
        templates: templates,
        pre_fft: true,
    }
}
