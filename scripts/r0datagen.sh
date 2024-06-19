#!/bin/bash
./scripts/datagen.sh 500000 40 ai/data/15x15/500k/r0/10-40_s50_Q50_turned2_wider+_oob_eq_Mc_ii_wbhnv/ -i i -s 50 -Q 50 -M c -w b -w n -w h -w v --gpu 0 r 0
./scripts/datagen.sh 2000000 40 ai/data/15x15/2m/r0/10-40_s50_Q50_turned2_wider+_oob_eq_Mt_ii_wbhnv/ -i i -s 50 -Q 50 -M t -w b -w n -w h -w v --gpu 1 r 0 
./scripts/datagen.sh 2000000 40 ai/data/15x15/2m/r0/10-40_s50_Q50_turned2_wider+_oob_eq_Mr_ii_wbhnv/ -i i -s 50 -Q 50 -M r -w b -w n -w h -w v --gpu 2 r 0 
