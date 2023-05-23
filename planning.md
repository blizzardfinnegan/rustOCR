# Planning document
--- 
## New plan

### Config class
same as before, but object oriented

### image parsing class
same as before, but more object-oriented

### saving data class
same as before, but more object oriented. Include lanternfish struct in here

### AOL fixture class
similar to before, but make GPIO access safer. 

### Main loop
same as before

--- 
## Current design

### Main testing loop

```
Wake devices
create Lanternfish object for all devices
for iterationCount:
    for each active camera:
        open a new thread
        process the image (with OpenCV) from the camera into a file
        process the file (with Tesseract) into a number
        add to Lanterfish object and local list
    join to all above threads

    check all values for bad values
        if fail, reset for 20s, try again
    
    write vales to output file
```

### AOL fixture class

background run thread to pause movement based on Run/Pause switch.
```
    poll every 0.1s, block if switch is high
```

3 input pins (upper limit, lower limit, run switch)
3 output pins (motor on, motor directon, piston activation)
motor movement timeout 3s
GPIO poll time 0.01s

internal reset arm function:
```
    if the arm is already at the upper limit switch, lower it for 0.5s
    send the motor up
    poll for the upper limit switch to turn on
    once upper limit switch is on, or if timeout, stop the motor
    if timeout, error; else return how long it took to get back
```

internal find distance between limit switches function
```
    reset arm
    if the reset arm failed, catch the error, throw a new one [break]
    send the motor down
    count polls to lower limit switch
    once lower limit switch is on, or timeout, stop motor

    if timeout warn user of speed being too slow [break]

    send the motor up
    count polls to upper limit switch
    once upper switch is on, or timeout, stop motor
    
    if timeout warn user of speed being too slow [break]

    set travel distance as the lesser of the two counts
```

internal go to limit switch function
```
    set direction based on passed boolean
    travel 95% of the way to the limit switch
    output whether travel was successful
```

external travel up and down functions (wrappers around go to limit function)

external press button function (push button for one second)

external iteration movement function (go up, go down, wait one poll, press the button)

### Saving data class

output file location basd on date and time (down to minute)

write values function
```
    for each device in Lanternfish object (each device has a map of values and value counts):
        get the list of values, sorted
        for each value:
            increase the iteration count amount by the value count amount
            if its a good value, increase the pass count value by the same amount
            add (value * count) to the total sum
        [see java code for part 1 of std.dev implementation]
        mean = sum / iteration count
        [find median]
        [see java code for part 2 of std.dev implementation]

        set the value in the config object for this device for std.dev, total iteraions, pass iterations, and median value
    save to file
```

### Lanternfish object class

### Config class

function to 
- get value from config
- set value in config
- save current config
- save default config
- save single device default config
- load current config
- load default config
- set config save location [future]

### Image parsing

create a tesseract api for each camera object
each camera object needs to be a video object, where you grab individual frames from the device

internal wrapper function to:
- take picture
- take burst of pictures
- save picture
- crop image
- threshold image
- compose multiple images on top of each other via bitwise and

public functions to:
- turn an image file into a double
- camera name in, file out
- show image to user
- allow user to set crop region using highgui

