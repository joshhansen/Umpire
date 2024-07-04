#!/bin/bash
./scripts/datagen.sh 1000000 40 ai/data/15x15/500k/RR/10-40_s50_Q50_turned2_actioned_wider+_oob_eq_Mc_ii_wbhnv/ -i i -s 50 -Q 50 -M c -w b -w n -w h -w v --gpu 0 RR
./scripts/datagen.sh 1000000 40 ai/data/15x15/500k/RR/10-40_s50_Q50_turned2_actioned_wider+_oob_eq_Mc_iv_id_wbhnv/ -i v -id -s 50 -Q 50 -M c -w b -w n -w h -w v --gpu 0 RR
./scripts/datagen.sh 5000000 40 ai/data/15x15/2m/RR/10-40_s50_Q50_turned2_actioned_wider+_oob_eq_Mt_ii_wbhnv/ -i i -s 50 -Q 50 -M t -w b -w n -w h -w v --gpu 1 RR 
./scripts/datagen.sh 10000 40 ai/data/15x15/500k/RR/10-40_s50_Q50_turned2_actioned_wider+_oob_eq_Mt_iv_id_wbhnv/ -i v -id -s 50 -Q 50 -M t -w b -w n -w h -w v --gpu 0 RR
./scripts/datagen.sh 8000000 40 ai/data/15x15/2m/RR/10-40_s50_Q50_turned2_actioned_wider+_oob_eq_Mr_ii_wbhnv/ -i i -s 50 -Q 50 -M r -w b -w n -w h -w v --gpu 2 RR
./scripts/datagen.sh 300000 40 ai/data/15x15/500k/RR/10-40_s50_Q50_turned2_actioned_wider+_oob_eq_Mr_iv_id_wbhnv/ -i v -id -s 50 -Q 50 -M r -w b -w n -w h -w v --gpu 0 RR
