/*
 Author: Alan Pipitone
 Description: Simple Rust code to demonstrate template matching.
              For the sake of readability and simplicity, error handling
              is not optimal.
 Date: 23/10/2024
 Email: alan.pipitone@gmail.com
*/

use opencv::imgproc::COLOR_BGR2GRAY;
use opencv::prelude::*;
use opencv::{Result, core, highgui, imgcodecs, imgproc};

fn main() -> Result<()> {

    // Load source and template images; source is mutable because we will draw on it
    let mut source = imgcodecs::imread("source.png", imgcodecs::IMREAD_COLOR)?;

    let mut source_gray = core::Mat::default();

    // Convert the source image to grayscale
    imgproc::cvt_color(&source, &mut source_gray, COLOR_BGR2GRAY, 0)?;

    // Load the template image in grayscale
    let template = imgcodecs::imread("template.png", imgcodecs::IMREAD_GRAYSCALE)?;

    let mut result = core::Mat::default();
    let mask = core::Mat::default();

    // Perform normalized cross-correlation matching
    imgproc::match_template(&source_gray, &template, &mut result, imgproc::TM_CCOEFF_NORMED, &mask)?;

    // Set a threshold value for the matching result
    let threshold = 0.8;

    // Variables for the min_max_loc function
    let mut min_val = 0.0;
    let mut max_val = 0.0;
    let mut min_loc = core::Point::new(0, 0);
    let mut max_loc = core::Point::new(0, 0);

    let mut locations = Vec::new();

    // Loop to find and mask all matching locations until none remain above the threshold
    loop {
        // Find the min and max values and their locations in the result
        core::min_max_loc(&result, Some(&mut min_val), Some(&mut max_val), Some(&mut min_loc), Some(&mut max_loc), &mask)?; 
        
        let mut top_left = core::Point::new(max_loc.x - template.cols() / 2, max_loc.y - template.rows() / 2);
        let mut bottom_right = core::Point::new(max_loc.x + template.cols() / 2, max_loc.y + template.rows() / 2);

        // If the match is above the threshold, process it
        if max_val >= threshold {
            locations.push(max_loc);

            // Ensure coordinates are within valid bounds
            top_left.x = top_left.x.max(0);
            top_left.y = top_left.y.max(0);
            bottom_right.x = bottom_right.x.min(result.cols());
            bottom_right.y = bottom_right.y.min(result.rows());

            // Black out the ROI (the region where we found the current template)
            let region = core::Rect::from_points(top_left, bottom_right);
            result.roi_mut(region)?.set_scalar((0.0).into())?;

            // Draw a red rectangle on the source image
            imgproc::rectangle(&mut source, core::Rect::from_points(max_loc, core::Point::new(max_loc.x + template.cols(), max_loc.y + template.rows())), (0, 0, 255).into(), 2, 8, 0)?;

        } else {
            // No more templates found, exit the loop
            break;
        }
    }

    // Infinite loop to display the image on the screen
    loop {
        highgui::imshow("template detected", &source)?;
        let char = highgui::wait_key(10)?;

        // Exit the program when any key is pressed
        if char != -1
        {
            //let params = core::Vector::new();
            //imgcodecs::imwrite(&"output.png", &source, &params)?;
            break;
        }

    }
    highgui::destroy_all_windows()?;

    Ok(())
}
