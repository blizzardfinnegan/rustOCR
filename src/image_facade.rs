use std::{sync::Mutex, thread, collections::VecDeque};

use config::*;
#[allow(unused_imports)]
use opencv::{videoio::{VideoCapture, VideoCaptureTrait, CAP_PROP_FOURCC, 
                       CAP_ANY, VideoWriter, CAP_PROP_FRAME_WIDTH, 
                       CAP_PROP_FRAME_HEIGHT}, 
             imgproc::{THRESH_BINARY, COLOR_BGR2GRAY,threshold,cvt_color},
             core::{bitwise_and,Mat},
             highgui::{select_roi,imshow},
             imgcodecs::{imread,imwrite,imdecode}};
#[allow(unused_imports)]
use leptess::{leptonica::{pix_read,Pix},
              tesseract::TessApi};

const FRAME_WIDTH:i32 = 800;
const FRAME_HEIGHT:i32 = 600;
const MJPG_FOURCC_CODE:i32 = VideoWriter::fourcc('m','j','p','g').unwrap();

pub struct Camera{
    settings: Config,
    camera: FrameGrabber,
    name: String,
    ocr: TessApi,
}

struct FrameGrabber{
    camera: VideoCapture,
    image_queue: Mutex<VecDeque<Mat>>,
}

impl FrameGrabber{
    fn new(camera:VideoCapture, composite_frames:i64) -> Self{
        let mut output = Self{
            camera,
            image_queue: Mutex::new(VecDeque::with_capacity(composite_frames as usize))
        };
        thread::spawn(move||{
            loop {
                let mut image = Mat::default();
                let result = output.camera.read(&mut image);
                match result{
                    Ok(true) => {
                        let mut image_queue = *output.image_queue.lock().unwrap();
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

    fn grab(&self, mut image:&Mat){
        let mut image_queue = *self.image_queue.lock().unwrap();
        image = &mut image_queue.pop_front().unwrap();
    }

    fn burst(&self) -> Vec<Mat>{
        let mut image_queue = *self.image_queue.lock().unwrap();
        return image_queue.clone().into();
    }
}

impl Camera{
    pub fn new(camera_name:String) -> Option<Self>{
        let defaults = Config::builder();
        defaults.set_default("crop x", 275);
        defaults.set_default("crop y", 200);
        defaults.set_default("crop width", 80);
        defaults.set_default("crop height", 50);
        defaults.set_default("threshold value", 50);
        defaults.set_default("composite frames", 5);
        defaults.set_default("active", true);
        let default = defaults.build().unwrap();

        let settings = Config::builder().add_source(default).build().unwrap();

        let mut camera = VideoCapture::from_file(&camera_name, CAP_ANY).unwrap();
        camera.set(CAP_PROP_FOURCC, MJPG_FOURCC_CODE as f64);
        camera.set(CAP_PROP_FRAME_WIDTH, FRAME_WIDTH as f64);
        camera.set(CAP_PROP_FRAME_HEIGHT, FRAME_HEIGHT as f64);
        if let result = camera.open_file(&camera_name, CAP_ANY){
            match result {
                Ok(_) => {},
                Err(error) => return None
            }
        };
        let frame_grabber = FrameGrabber::new(camera, settings.get_int("composite frames").unwrap_or(1));

        let name = camera_name.split('-').last().unwrap().to_string();

        let ocr = TessApi::new(Some("tessdata"), "Pro6_temp_test").unwrap();

        Some(Self{
            settings,
            camera: frame_grabber,
            name,
            ocr,
        })
    }

    fn take_picture(&mut self) -> Mat{
        let mut image = Mat::default();
        self.camera.grab(&mut image);
        let mut output = Mat::default();
        cvt_color(&mut image, &mut output, COLOR_BGR2GRAY, 0);
        return output;
    }
}
