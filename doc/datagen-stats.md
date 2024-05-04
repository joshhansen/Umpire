# Datagen stats

## Bronte Mark II


### 2 May 2024 - 3b85d36ebcbc34a5ebd2bcd7abd2a3584b34a869
Running on 40 cores

10-40 dims

In blocks of 10000 games

I'm getting about 3000 games on a single process after 2 hours of runtime.

Per-process rate is: 3000 games / (2 * 60 * 60 seconds) = 0.4167 games / second

So overall rate is: 40 * 0.4167 games / second = 16.64 games / second

At this rate, generating 1 million games would take 16.7 hours

This will be longer in practice because the 100 work units of 10000 only run 40 at a time, so we will eventually be idling half of the 40, cutting the rate to more like 8.32 games / second.

We should do 40 cores of 25000 games each.

We could also profile datagen again and look for optimizations.
