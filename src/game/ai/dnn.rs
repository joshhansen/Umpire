use std::path::Path;


use lfa::{
    Approximator,
    Parameterised,
};

use ndarray::{
    ArrayBase,
    Array2,
    Dim,
};


use half::f16;

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
        WeightsView,
        WeightsViewMut,
        linear::{
            Features,
            error::{
                Error as LinearError,
                ErrorKind,
            },
        },
    },
};

use crate::{
    game::{
        Game,
        ai::UmpireAction,
    },
};

const ITS: usize = 1000;


pub struct DNN {
    session: Session,
    graph: Graph,
    features_1d: Operation,
    is_enemy_belligerent: Operation,
    is_observed: Operation,
    is_neutral: Operation,
    y: Operation,
    y_hat: Operation,
}

impl DNN {
    pub fn new() -> Result<Self,Box<dyn std::error::Error>> {

        let export_dir = "ai/umpire_regressor";
        if !Path::new(export_dir).exists() {
            return Err(Box::new(
                Status::new_set(
                    Code::NotFound,
                    &format!(
                        "Run 'python regression_savedmodel.py' to generate \
                         {} and try again.",
                        export_dir
                    ),
                )
                .unwrap(),
            ));
        }

        // Load the saved model exported by regression_savedmodel.py.
        let mut graph = Graph::new();
        let session = Session::from_saved_model(
            &SessionOptions::new(),
            &["train", "serve"],
            &mut graph,
            export_dir,
        )?;

        for func in graph.get_functions().unwrap().iter() {
            println!("{:?}", func);
        }

        
        let features_1d = graph.operation_by_name_required("1d_features")?;
        let is_enemy_belligerent = graph.operation_by_name_required("is_enemy_belligerent")?;
        let is_observed = graph.operation_by_name_required("is_observed")?;
        let is_neutral = graph.operation_by_name_required("is_neutral")?;

        let y = graph.operation_by_name_required("y")?;

        let y_hat = graph.operation_by_name_required("y_hat")?;

        Ok(Self {
            session,
            graph,
            features_1d,
            is_enemy_belligerent,
            is_observed,
            is_neutral,
            y,
            y_hat,
        })
    }
}

// pub trait Parameterised {
//     fn weights_view(&self) -> WeightsView;
//     fn weights_view_mut(&mut self) -> WeightsViewMut;

//     fn weights(&self) -> Weights { ... }
//     fn weights_dim(&self) -> [usize; 2] { ... }
// }

// impl Parameterised for DNN {
//     fn weights_view(&self) -> WeightsView {
//         self.weights.view()
//     }

//     //NOTE This is weird---we return a mutable view of a copy of the weights, not a mutable view of the actual weights
//     fn weights_view_mut(&mut self) -> WeightsViewMut {
//         self.weights.view_mut()
//     }
// }

// // pub trait Approximator: Parameterised {
// //     type Output;
// //     fn evaluate(&self, features: &Features) -> Result<Self::Output>;
// //     fn update<O: Optimiser>(
// //         &mut self, 
// //         optimiser: &mut O, 
// //         features: &Features, 
// //         error: &Self::Output
// //     ) -> Result<()>;

// //     fn n_outputs(&self) -> usize { ... }
// // }




// impl Approximator for DNN {
//     type Output = f64;

//     fn evaluate(&self, features: &Features) -> Result<Self::Output,LinearError> {
//         let mut output_step = SessionRunArgs::new();
//         let regressand_idx = output_step.request_fetch(&self.y_hat, 0);

//         self.session.run(&mut output_step)
//                     .map_err(|err| LinearError{kind: ErrorKind::Evaluation, message: format!("Error evaluating DNN: {}", err)})
        
//         ?;

//         let result = output_step.fetch(regressand_idx)
//                     .map_err(|err| LinearError{kind: ErrorKind::Evaluation, message: format!("Error evaluating DNN: {}", err)})
//         ?[0];

//         Ok(result)
//     }

//     fn update<O: Optimiser>(
//         &mut self, 
//         optimiser: &mut O, 
//         features: &Features, 
//         error: Self::Output
//     ) -> Result<(),LinearError> {
        
//         // Train the model (e.g. for fine tuning).
//         let mut train_step = SessionRunArgs::new();

//         //FIXME This is a horribly inefficient way of grabbing this data
//         let x: Vec<f16> = features.clone().expanded().into_iter()
//                                           .map(|x| f16::from_f64(*x)).collect();



        
//         //FIXME Splitting the input vector is something Keras should be doing but isn't quite ready to yet
//         let features_1d: Tensor<f16> = Tensor::new(&[self.features_1d.num_outputs() as u64])
//                                                         .with_values(&x[0..14])
//                                                         .map_err(|err| LinearError{
//                                                             kind: ErrorKind::Optimisation,
//                                                             message: format!("Error training DNN: {}", err)
//                                                         })?;
        
//         let mut base: usize = features_1d.dims()[0] as usize;

//         let is_enemy_belligerent: Tensor<f16> = Tensor::new(&[self.is_enemy_belligerent.num_outputs() as u64])
//                                                         .with_values(&x[base..(base+121)])
//                                                         .map_err(|err| LinearError{
//                                                             kind: ErrorKind::Optimisation,
//                                                             message: format!("Error training DNN: {}", err)
//                                                         })?;

//         base += 121;

//         let is_observed: Tensor<f16> = Tensor::new(&[self.is_observed.num_outputs() as u64])
//                                                         .with_values(&x[base..(base+121)])
//                                                         .map_err(|err| LinearError{
//                                                             kind: ErrorKind::Optimisation,
//                                                             message: format!("Error training DNN: {}", err)
//                                                         })?;

//         base += 121;


//         let is_neutral: Tensor<f16> = Tensor::new(&[self.is_neutral.num_outputs() as u64])
//                                                         .with_values(&x[base..(base+121)])
//                                                         .map_err(|err| LinearError{
//                                                             kind: ErrorKind::Optimisation,
//                                                             message: format!("Error training DNN: {}", err)
//                                                         })?;

//         // let mut regressand: Tensor<f16> = Tensor::new(&[self.features_1d.num_outputs() as u64]);

//         // train_step.add_feed(&op_x, 0, &x);
//         // train_step.add_feed(&op_y, 0, &y);
//         // train_step.add_target(&op_train);

//         //TODO populate the tensors

        




//         train_step.add_feed(&self.features_1d, 0, &features_1d);
//         train_step.add_feed(&self.is_enemy_belligerent, 0, &is_enemy_belligerent);
//         train_step.add_feed(&self.is_observed, 0, &is_observed);
//         train_step.add_feed(&self.is_neutral, 0, &is_neutral);

//         //FIXME
//         // train_step.add_target(&self.train);

//         for _ in 0..ITS {
//             self.session.run(&mut train_step)
//                    .map_err(|err| LinearError{kind: ErrorKind::Optimisation, message: format!("Error training DNN: {}", err)})?;
//         }

//         Ok(())
//     }
// }


// pub trait Optimiser<G = Features> {
//     fn step(&mut self, weights: &mut ArrayViewMut1<f64>, features: &G, loss: f64) -> Result<()>;

//     fn step_batch(&mut self, weights: &mut ArrayViewMut1<f64>, samples: &[(G, f64)]) -> Result<()> {
//         samples
//             .into_iter()
//             .map(|(g, e)| self.step(weights, g, *e))
//             .collect()
//     }

//     fn reset(&mut self) {}
// }


// /// An interface for state-action value functions.
// pub trait StateActionFunction<X: ?Sized, U: ?Sized> {
//     type Output;

//     fn evaluate(&self, state: &X, action: &U) -> Self::Output;

//     fn update(&mut self, state: &X, action: &U, error: Self::Output);
// }

// pub trait EnumerableStateActionFunction<X: ?Sized>:
//     StateActionFunction<X, usize, Output = f64>
// {
//     fn n_actions(&self) -> usize;

//     fn evaluate_all(&self, state: &X) -> Vec<f64>;

//     fn update_all(&mut self, state: &X, errors: Vec<f64>);

//     fn find_min(&self, state: &X) -> (usize, f64) {
//         let mut iter = self.evaluate_all(state).into_iter().enumerate();
//         let first = iter.next().unwrap();

//         iter.fold(first, |acc, (i, x)| if acc.1 < x { acc } else { (i, x) })
//     }

//     fn find_max(&self, state: &X) -> (usize, f64) {
//         let mut iter = self.evaluate_all(state).into_iter().enumerate();
//         let first = iter.next().unwrap();

//         iter.fold(first, |acc, (i, x)| if acc.1 > x { acc } else { (i, x) })
//     }
// }

impl StateActionFunction<Game, usize> for DNN {
    type Output = f64;

    fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
        //TODO
    }

    fn update(&mut self, state: &Game, action: &usize, error: Self::Output) {
        //TODO
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