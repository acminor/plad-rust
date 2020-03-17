use crate::cli::DCNorm;
use crate::utils;

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

arg_enum! {
    #[derive(Clone, Copy)]
    pub enum TemplateNorm {
        None,
        // normalize power over different length templates
        // - necessary as window length is less than template length by a lot
        LengthNorm,
    }
}

pub fn parse_template_file(file_name: String, template_group_sz: usize,
                           dc_norm: DCNorm, template_norm: TemplateNorm) -> Templates {
    let contents = fs::read_to_string(&file_name)
        .expect("Failed to read Templates TOML file");

    let template_toml: TemplateToml =
        toml::from_str(&contents).expect("Failed to parse Templates TOML file");

    let templates: Vec<TemplateGroup> = {
        let toml_templates_file = utils::normalize_local_data_paths(&file_name,
                                                                    &template_toml.templates);
        let mut file = fs::File::open(&toml_templates_file)
            .expect(&format!("Failed to read Templates templates file {}", toml_templates_file));
        let mut contents: Vec<u8> = Vec::new();
        file.read_to_end(&mut contents)
            .expect("Failed reading contents of templates.");

        let mut de = rmp_serde::Deserializer::new(&contents[..]);

        let temp: Vec<Vec<f32>> = serde::Deserialize::deserialize(&mut de)
            .expect("Failed to deserialize templates");

        let mut max_len = temp
            .iter()
            .map(|template| template.len())
            .max()
            .expect("Issue getting max template set length.");

        // XXX hack for our current template settings
        //     to prevent a factor >= 13 from appearing
        //     in the prime factorization of an fft dimenion
        //     - this does not work with the arrayfire opencl fft
        //       operation https://github.com/arrayfire/arrayfire-python/issues/139
        // FIXME implement better fix by checking prime factors for this
        //match AF::get_active_backend()  {
        //    AF::Backend::OPENCL => {
        //        max_len = 6000;
        //    }
        //    _ => {}
        //}
        //
        // NOTE FOR NOW FIXED IN TEMPLATE GENERATION BY ROUNDING TO NEAREST POWER OF TWO LENGTH
        // - SO WE DO THE SAME HERE

        let max_len = (2 as usize).pow((max_len as f32).log2().ceil() as u32);
        //let max_len = 8192;
        println!("FFT length: {}", max_len);

        // using the numpy fftfreq reference
        // [ ] TODO check if correct
        // - ie only concerned with pos. freq. in fft
        let real_len: usize = if max_len % 2 == 1 {
            // odd
            (max_len - 1) / 2
        } else {
            // even
            max_len / 2 - 1
        };

        temp.chunks(template_group_sz)
            .map(|chunk| {
                let chunk_len = chunk.len();

                let mut chunk: Vec<AF_Array<Complex<f32>>> = chunk
                    .iter()
                    .map(|template| {
                        let template_length = template.len();
                        let template = AF_Array::new(
                            &template,
                            AF_Dim4::new(&[template.len() as u64, 1, 1, 1]),
                        );

                        let template = match template_norm {
                            TemplateNorm::LengthNorm => {
                                let norm_factor = (template_length as f32) / (max_len as f32);
                                AF::mul(
                                    &template,
                                    &norm_factor,
                                    false
                                )
                            }
                            _ => {
                                template
                            }
                        };

                        // NOTE Remove DC constant of template to focus on signal
                        //      - This is very important and will lead to false
                        //        detection or searching for the wrong signal
                        let template = match dc_norm {
                            DCNorm::MeanRemoveTemplate
                            | DCNorm::MeanRemoveTemplateAndStar
                            | DCNorm::HistMeanRemoveStarAndTemplate
                            | DCNorm::NormAtZeroStarAndMeanRemoveTemplate => {
                                let template_mean = AF::mean(&template, 0);
                                AF::sub(&template, &template_mean, false)
                            }
                            DCNorm::NormAtZeroTemplate
                            | DCNorm::NormAtZeroTemplateAndStar
                            | DCNorm::HistMeanRemoveStarAndNormAtZeroTemplate
                            | DCNorm::MeanRemoveConstBumpStarAndNormAtZeroTemplate =>
                            {
                                let template_adjustment = AF::min(&template, 0);
                                AF::sub(&template, &template_adjustment, false)
                            }
                            _ => template,
                        };

                        //AF::device_gc();

                        //println!("template length {}", template.elements());
                        //println!("max length {}", max_len);
                        let fft_bs = AF::fft(&template, 1.0, max_len as i64);
                        //AF::device_gc();
                        let temp = AF::rows(&fft_bs, 0, real_len as u64);
                        //AF::device_gc();
                        //AF::conjg(&temp)
                        temp
                    })
                    .collect();

                let mut chunk = chunk.drain(0..chunk.len());
                let chunk_out = {
                    let mut chunk_out = chunk
                        .next()
                        .expect("Should have at least one template.");
                    for lchunk in chunk {
                        //AF::device_gc();
                        chunk_out = AF::join(1, &chunk_out, &lchunk);
                    }
                    //AF::device_gc();

                    chunk_out
                };

                //let mut buf: Vec<Complex<f32>> = Vec::new();
                //buf.resize(chunk_out.elements(), Complex::new(0.0, 0.0 as f32));
                //chunk_out.lock();
                //chunk_out.host(&mut buf);
                //chunk_out.unlock();

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
        templates,
        pre_fft: true,
    }
}
