use std::{ffi::OsStr, path::Path};


use tensorflow::{
    Code,
    Graph,
    Operation,
    Session,
    SessionOptions,
    SessionRunArgs,
    Status,
    Tensor,
};

use rsrl::{
    fa::{
        EnumerableStateActionFunction,
        StateActionFunction,
    },
};

use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    game::{
        Game,
        ai::UmpireAction, player::{TurnTaker, LimitedTurnTaker},
    },
};

const ITS: usize = 1000;


// #[derive(Deserialize,Serialize)]
#[derive(Debug)]
pub struct DNN {
    session: Session,
    graph: Graph,

    // The inputs
    features_1d: Operation,
    is_enemy_belligerent: Operation,
    is_observed: Operation,
    is_neutral: Operation,

    /// The true value of each action (the discounted estimate, anyway)
    action_values: Operation,

    /// The predicted value of each action
    action_values_hat: Operation,

    // Operations
    action_train_ops: Vec<Operation>,
}

impl DNN {
    pub fn load_from_dir(path: &Path) -> Result<Self,Box<dyn std::error::Error>> {
        // let export_dir = "ai/umpire_regressor";

        if !path.exists() {
            return Err(Box::new(
                Status::new_set(
                    Code::NotFound,
                    &format!(
                        "Run 'python regression_savedmodel.py' to generate \
                         {} and try again.",
                         path.display()
                    ),
                )
                .unwrap(),
            ));
        }

        // Load the saved model exported by regression_savedmodel.py.
        let mut graph = Graph::new();
        let session = Session::from_saved_model(
            &SessionOptions::new(),
            // &["train", "serve"],
            &["serve"],
            &mut graph,
            path,
        )?;

        // for func in graph.get_functions().unwrap().iter() {
        //     println!("{:?}", func);
        // }

        
        let features_1d = graph.operation_by_name_required("1d_features")?;
        let is_enemy_belligerent = graph.operation_by_name_required("is_enemy_belligerent")?;
        let is_observed = graph.operation_by_name_required("is_observed")?;
        let is_neutral = graph.operation_by_name_required("is_neutral")?;

        let action_values = graph.operation_by_name_required("action_values")?;
        let action_values_hat = graph.operation_by_name_required("action_values_hat")?;

        let action_train_ops = UmpireAction::possible_actions().iter().enumerate().map(|(i,action)| {
            graph.operation_by_name_required(format!("train_action_{}", i).as_str()).unwrap()
        }).collect();

        Ok(Self {
            session,
            graph,
            features_1d,
            is_enemy_belligerent,
            is_observed,
            is_neutral,
            action_values,
            action_values_hat,
            action_train_ops,
        })
    }

    // pub fn new() -> Result<Self,Box<dyn std::error::Error>> {

    //     let export_dir = "ai/umpire_regressor";
    //     if !Path::new(export_dir).exists() {
    //         return Err(Box::new(
    //             Status::new_set(
    //                 Code::NotFound,
    //                 &format!(
    //                     "Run 'python regression_savedmodel.py' to generate \
    //                      {} and try again.",
    //                     export_dir
    //                 ),
    //             )
    //             .unwrap(),
    //         ));
    //     }

    //     // Load the saved model exported by regression_savedmodel.py.
    //     let mut graph = Graph::new();
    //     let session = Session::from_saved_model(
    //         &SessionOptions::new(),
    //         &["train", "serve"],
    //         &mut graph,
    //         export_dir,
    //     )?;

    //     for func in graph.get_functions().unwrap().iter() {
    //         println!("{:?}", func);
    //     }

        
    //     let features_1d = graph.operation_by_name_required("1d_features")?;
    //     let is_enemy_belligerent = graph.operation_by_name_required("is_enemy_belligerent")?;
    //     let is_observed = graph.operation_by_name_required("is_observed")?;
    //     let is_neutral = graph.operation_by_name_required("is_neutral")?;

    //     let action_values = graph.operation_by_name_required("action_values")?;
    //     let action_values_hat = graph.operation_by_name_required("action_values_hat")?;

    //     let action_train_ops = UmpireAction::possible_actions().iter().enumerate().map(|(i,action)| {
    //         graph.operation_by_name_required(format!("train_action_{}", i).as_str()).unwrap()
    //     }).collect();

    //     Ok(Self {
    //         session,
    //         graph,
    //         features_1d,
    //         is_enemy_belligerent,
    //         is_observed,
    //         is_neutral,
    //         action_values,
    //         action_values_hat,
    //         action_train_ops,
    //     })
    // }

    fn tensors_for(&self, state: &Game) -> (Tensor<f64>,Tensor<f64>,Tensor<f64>,Tensor<f64>) {
        let x = state.features();

        //FIXME Splitting the input vector is something Keras should be doing but isn't quite ready to yet
        let features_1d: Tensor<f64> = Tensor::new(&[self.features_1d.num_outputs() as u64])
                                                        .with_values(&x[0..14])
                                                        .unwrap();
                                                        // .map_err(|err| LinearError{
                                                        //     kind: ErrorKind::Optimisation,
                                                        //     message: format!("Error training DNN: {}", err)
                                                        // })?;
        
        let mut base: usize = features_1d.dims()[0] as usize;

        let is_enemy_belligerent: Tensor<f64> = Tensor::new(&[self.is_enemy_belligerent.num_outputs() as u64])
                                                        .with_values(&x[base..(base+121)])
                                                        .unwrap();
                                                        // .map_err(|err| LinearError{
                                                        //     kind: ErrorKind::Optimisation,
                                                        //     message: format!("Error training DNN: {}", err)
                                                        // })?;

        base += 121;

        let is_observed: Tensor<f64> = Tensor::new(&[self.is_observed.num_outputs() as u64])
                                                        .with_values(&x[base..(base+121)])
                                                        .unwrap();
                                                        // .map_err(|err| LinearError{
                                                        //     kind: ErrorKind::Optimisation,
                                                        //     message: format!("Error training DNN: {}", err)
                                                        // })?;

        base += 121;


        let is_neutral: Tensor<f64> = Tensor::new(&[self.is_neutral.num_outputs() as u64])
                                                        .with_values(&x[base..(base+121)])
                                                        .unwrap();
                                                        // .map_err(|err| LinearError{
                                                        //     kind: ErrorKind::Optimisation,
                                                        //     message: format!("Error training DNN: {}", err)
                                                        // })?;

        // train_step.add_target(&op_train);

        
        (features_1d, is_enemy_belligerent, is_observed, is_neutral)
    }
}


impl StateActionFunction<Game, usize> for DNN {
    type Output = f64;

    fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
        
        let (features_1d, is_enemy_belligerent, is_observed, is_neutral) = self.tensors_for(state);

        let mut output_step = SessionRunArgs::new();

        output_step.add_feed(&self.features_1d, 0, &features_1d);
        output_step.add_feed(&self.is_enemy_belligerent, 0, &is_enemy_belligerent);
        output_step.add_feed(&self.is_observed, 0, &is_observed);
        output_step.add_feed(&self.is_neutral, 0, &is_neutral);


        let regressand_idx = output_step.request_fetch(&self.action_values_hat, *action as i32);

        self.session.run(&mut output_step)
                    .unwrap();
                    // .map_err(|err| LinearError{kind: ErrorKind::Evaluation, message: format!("Error evaluating DNN: {}", err)})
        
        // ?;

        let result = output_step.fetch(regressand_idx)
                    // .map_err(|err| LinearError{kind: ErrorKind::Evaluation, message: format!("Error evaluating DNN: {}", err)})
                    .unwrap().get(&[0]);
        // ?[0];

        result


    }

    fn update(&mut self, state: &Game, action: &usize, error: Self::Output) {

        // The estimate of the action value the model currently generates
        let action_value_hat = self.evaluate(state, action);

        // Use that estimate and the reported error to reconstruct what the "actual" action value was
        let action_value = action_value_hat + error;


        let (features_1d, is_enemy_belligerent, is_observed, is_neutral) = self.tensors_for(state);

        // train_step.add_target(&op_train);

        let action_value_tensor: Tensor<f64> = Tensor::new(&[1_u64])
                                                        .with_values(&[action_value])
                                                        .unwrap();


        let mut train_step = SessionRunArgs::new();

        // Set inputs
        train_step.add_feed(&self.features_1d, 0, &features_1d);
        train_step.add_feed(&self.is_enemy_belligerent, 0, &is_enemy_belligerent);
        train_step.add_feed(&self.is_observed, 0, &is_observed);
        train_step.add_feed(&self.is_neutral, 0, &is_neutral);

        // Set the correct output
        train_step.add_feed(&self.action_values, *action as i32, &action_value_tensor);
        

        let regressand_op = self.action_train_ops.get(*action).unwrap();

        train_step.add_target(&regressand_op);

        //FIXME
        // train_step.add_target(&self.train);

        for _ in 0..ITS {
            self.session.run(&mut train_step)
                        .unwrap();
                //    .map_err(|err| LinearError{kind: ErrorKind::Optimisation, message: format!("Error training DNN: {}", err)})?;
        }
    }
}


impl EnumerableStateActionFunction<Game> for DNN {
    fn n_actions(&self) -> usize {
        UmpireAction::possible_actions().len()
    }

    fn evaluate_all(&self, state: &Game) -> Vec<f64> {
        (0..self.n_actions()).map(|action_idx| {
            self.evaluate(state, &action_idx)
        }).collect()
    }

    //FIXME Is this right?
    fn update_all(&mut self, state: &Game, errors: Vec<f64>) {
        for error in errors {
            for action_idx in 0..self.n_actions() {
                self.update(state, &action_idx, error);
            }
        }
    }
}


impl TurnTaker for DNN {
    fn take_turn_not_clearing(&mut self, game: &mut Game) {
        unimplemented!()
    }

    fn take_turn_clearing(&mut self, game: &mut Game) {
        unimplemented!()
    }
}



// fn main() -> Result<(), Box<dyn Error>> {


//     // Generate some test data.
//     let w = 0.1;
//     let b = 0.3;
//     let num_points = 100;
//     let steps = 201;
//     // let mut rand = random::default();
//     let mut x = Tensor::new(&[num_points as u64]);
//     let mut y = Tensor::new(&[num_points as u64]);
//     for i in 0..num_points {
//         x[i] = (2.0 * rand.read::<f64>() - 1.0) as f32;
//         y[i] = w * x[i] + b;
//     }

//     // Load the saved model exported by regression_savedmodel.py.
//     let mut graph = Graph::new();
//     let session = Session::from_saved_model(
//         &SessionOptions::new(),
//         &["train", "serve"],
//         &mut graph,
//         export_dir,
//     )?;
//     let op_x = graph.operation_by_name_required("x")?;
//     let op_y = graph.operation_by_name_required("y")?;
//     let op_train = graph.operation_by_name_required("train")?;
//     let op_w = graph.operation_by_name_required("w")?;
//     let op_b = graph.operation_by_name_required("b")?;

//     // Train the model (e.g. for fine tuning).
//     let mut train_step = SessionRunArgs::new();
//     train_step.add_feed(&op_x, 0, &x);
//     train_step.add_feed(&op_y, 0, &y);
//     train_step.add_target(&op_train);
//     for _ in 0..steps {
//         session.run(&mut train_step)?;
//     }

//     // Grab the data out of the session.
//     let mut output_step = SessionRunArgs::new();
//     let w_ix = output_step.request_fetch(&op_w, 0);
//     let b_ix = output_step.request_fetch(&op_b, 0);
//     session.run(&mut output_step)?;

//     // Check our results.
//     let w_hat: f32 = output_step.fetch(w_ix)?[0];
//     let b_hat: f32 = output_step.fetch(b_ix)?[0];
//     println!(
//         "Checking w: expected {}, got {}. {}",
//         w,
//         w_hat,
//         if (w - w_hat).abs() < 1e-3 {
//             "Success!"
//         } else {
//             "FAIL"
//         }
//     );
//     println!(
//         "Checking b: expected {}, got {}. {}",
//         b,
//         b_hat,
//         if (b - b_hat).abs() < 1e-3 {
//             "Success!"
//         } else {
//             "FAIL"
//         }
//     );
//     Ok(())
// }