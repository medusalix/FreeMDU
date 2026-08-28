### VEROROUTE examples

**2026-AUG-28** - Some experiments

I have made two paneles, similar to those are below. One is working by **150k x 22pf** feedback, other is working by **330k x 10pf**. They were tested by **checkforerror**. It shows that the second have twice more sensitivite and still stable. It was no problem if I pushed the sensor in front of the LED or leaving distance. I also wached input signal (TX pin) on oscilloscope, and the picture was good for me.
Instead of using fix resistor value for setting the non-invering input of opamp I used a 10k potentiometer. 
I read back the setting:

```
first (150k x 22p):  9,36k - 9,14k - 290 
second (330k x 10p): 9,41k - 9,17k - 291
```

Next step is checking again with a washing machine.

[veroroute](https://sourceforge.net/projects/veroroute/)

![examples for ESP32-C3 and ESP32-C6](examples.png)

