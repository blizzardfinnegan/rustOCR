use std::sync::Arc;
use std::{sync::Mutex, thread, collections::VecDeque};

use config::*;
use opencv::imgcodecs::{IMWRITE_PNG_BILEVEL, IMREAD_UNCHANGED};
use opencv::{videoio::{VideoCapture, VideoCaptureTrait, CAP_PROP_FOURCC, 
                       CAP_ANY, VideoWriter, CAP_PROP_FRAME_WIDTH, 
                       CAP_PROP_FRAME_HEIGHT}, 
             imgproc::{THRESH_BINARY, COLOR_BGR2GRAY,threshold,cvt_color},
             core::{bitwise_and,Rect_,Mat,MatTraitConst,no_array,Vector},
             highgui::{select_roi,imshow},
             imgcodecs::{imwrite,imread}};
use leptess::LepTess;

const FRAME_WIDTH:i32 = 800;
const FRAME_HEIGHT:i32 = 600;
const CROP_X:&str = "crop x";
const CROP_Y:&str = "crop y";
const CROP_WIDTH:&str = "crop width";
const CROP_HEIGHT:&str = "crop height";
const THRESHOLD:&str = "threshold value";
const COMPOSITE_FRAMES:u16 = 5;

pub struct Camera{
    settings: Config,
    camera: FrameGrabber,
    name: String,
    ocr: LepTess,
    active: bool
}

struct FrameGrabber{
    image_queue: Arc<Mutex<VecDeque<Mat>>>,
}

impl FrameGrabber{
    fn new(mut camera:VideoCapture, composite_frames:u16) -> Self{
        let output = Self{
            image_queue: Arc::new(Mutex::new(VecDeque::with_capacity(composite_frames as usize)))
        };
        let image_queue = output.image_queue.clone();
        thread::spawn(move||{
            loop {
                let mut image = Mat::default();
                let result = &camera.read(&mut image);
                match result{
                    Ok(true) => {
                        let mut converted_image = Mat::default();
                        _ = cvt_color(&mut image, &mut converted_image, COLOR_BGR2GRAY, 0);
                        let mut image_queue = image_queue.lock().unwrap();
                        if image_queue.len() == image_queue.capacity(){
                            image_queue.pop_back();
                        }
                        image_queue.push_front(image);
                    }
                    _ => { break; }
                }           
            }
        });
        return output;
    }

    fn grab(&self) -> Option<Mat>{
        let mut image_queue = self.image_queue.lock().unwrap();
        image_queue.pop_front()
    }

    fn burst(&self) -> Vec<Mat>{
        let image_queue = self.image_queue.lock().unwrap();
        return image_queue.clone().into();
    }

    fn change_composite_frames(&self, composite_frames:u16){
        let mut image_queue = self.image_queue.lock().unwrap();
        image_queue.resize(composite_frames as usize, Default::default());
    }
}

impl Camera{
    pub fn new(camera_name:String) -> Option<Self>{
        let mut defaults = Config::builder();
        defaults = defaults.set_default(CROP_X, 275).unwrap();
        defaults = defaults.set_default(CROP_Y, 200).unwrap();
        defaults = defaults.set_default(CROP_WIDTH, 80).unwrap();
        defaults = defaults.set_default(CROP_HEIGHT, 50).unwrap();
        defaults = defaults.set_default(THRESHOLD, 50).unwrap();
        let default = defaults.build().unwrap();

        let settings = Config::builder().add_source(default).build().unwrap();

        let mut camera = VideoCapture::from_file(&camera_name, CAP_ANY).unwrap();
        _ = camera.set(CAP_PROP_FOURCC, VideoWriter::fourcc('m','j','p','g').unwrap() as f64);
        _ = camera.set(CAP_PROP_FRAME_WIDTH, FRAME_WIDTH as f64);
        _ = camera.set(CAP_PROP_FRAME_HEIGHT, FRAME_HEIGHT as f64);
        match camera.open_file(&camera_name, CAP_ANY) {
            Ok(_) => {},
            Err(_) => return None
        };
        let frame_grabber = FrameGrabber::new(camera, COMPOSITE_FRAMES);

        let name = camera_name.split('-').last().unwrap().to_string();

        let ocr = LepTess::new(Some("tessdata"), "Pro6_temp_test").unwrap();

        Some(Self{
            settings,
            camera: frame_grabber,
            name,
            ocr,
            active: true,
        })
    }

    pub fn show_image(&self){
        _ = imshow("Test image", &imread(&self.complete_process(),IMREAD_UNCHANGED).unwrap());
    }

    fn complete_process(&self) -> String{
        let images = self.camera.burst();
        let mut final_image = images[0].clone();
        for mut image in images{
            image = self.crop(image);
            image = self.threshold(image);
            _ = bitwise_and(&final_image.clone(), &image, &mut final_image, &no_array());
        }
        return self.save(final_image).unwrap_or("".to_string());
    }

    fn crop(&self,image:Mat) -> Mat{
        let x = self.settings.get_int(&CROP_X).unwrap();
        let y = self.settings.get_int(&CROP_Y).unwrap();
        let width = self.settings.get_int(&CROP_WIDTH).unwrap();
        let height = self.settings.get_int(&CROP_HEIGHT).unwrap();
        let roi = Rect_::new(x as i32, y as i32, width as i32, height as i32);
        return image.apply_1(roi).unwrap();
    }

    fn threshold(&self,image:Mat) -> Mat{
        let mut output = image.clone();
        let threshold_value = self.settings.get_int(&THRESHOLD).unwrap();
        _ = threshold(&image,&mut output, threshold_value as f64, 255 as f64 ,THRESH_BINARY);
        return Mat::default();
    }

    fn save(&self,image:Mat) -> Result<String,opencv::Error>{
        let mut write_parameters:Vector<i32> = Vector::new();
        write_parameters.push(IMWRITE_PNG_BILEVEL);
        let mut filename:String = String::new();
        filename.push_str(&chrono::Local::now().to_rfc3339().to_string());
        filename.push_str(&self.name);
        match imwrite(&filename, &image, &write_parameters){
            Ok(_) => Ok(filename),
            Err(error) => Err(error)
        }
    }

    pub fn set_crop(&mut self) -> Result<(),opencv::Error>{
        let temp = &self.camera.grab();
        match temp{
            Some(image) => {
                match select_roi(image, false, false){
                    Err(error) => { return Err(error); }
                    Ok(rect) => {
                        let mut new_settings = Config::builder().add_source(self.settings.clone());
                        new_settings = new_settings.set_override(CROP_X, rect.x).unwrap();
                        new_settings = new_settings.set_override(CROP_Y, rect.y).unwrap();
                        new_settings = new_settings.set_override(CROP_WIDTH, rect.width).unwrap();
                        new_settings = new_settings.set_override(CROP_HEIGHT, rect.height).unwrap();
                        self.settings = new_settings.build().unwrap();
                        Ok(())
                    }
                }
            }
            None => {
                log::error!("Image unavailable!");
                Err(opencv::Error { code: 0, message:"Invalid image!".to_string() })
            }
        }
    }

    pub fn set_threshold(&mut self, thresh:u16){
        let new_settings = Config::builder().add_source(self.settings.clone()).set_override(THRESHOLD, thresh);
        self.settings = new_settings.expect("Bad config settings!").build().unwrap();
    }

    pub fn set_composite_frames(&self, composite_frames:u16){
        self.camera.change_composite_frames(composite_frames);
    }

    pub fn deactivate(&mut self){ self.active = false; }
    pub fn activate(&mut self)  { self.active = true; }
    pub fn is_active(&self) -> bool { self.active }

    pub fn parse_image(&mut self, file_location:String) -> f64{
        _ = self.ocr.set_image(file_location);
        return str::parse(&self.ocr.get_utf8_text().unwrap()).unwrap()
    }
}
