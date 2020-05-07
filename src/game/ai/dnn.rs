use std::{fmt, path::Path};


use tensorflow::{
    Code,
    Graph,
    ImportGraphDefOptions,
    Operation,
    SavedModelBuilder,
    Scope,
    Session,
    SessionOptions,
    SessionRunArgs,
    Status,
    Tensor, SavedModelBundle,
};

use rsrl::{
    fa::{
        EnumerableStateActionFunction,
        StateActionFunction,
    },
};

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
    de::{self,Visitor},
};

use crate::{
    game::{
        Game,
        ai::UmpireAction, fX,
    },
};

use super::{Storable, Loadable};

const ITS: usize = 1000;
static TAG: &'static str = "serve";

struct BytesVisitor;
impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an array of bytes")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> where E: de::Error {
        Ok(Vec::from(v))
    }
}

#[derive(Debug)]
pub struct DNN {
    // Namespacey container for everything
    scope: Scope,

    session: Session,

    // The inputs
    features_1d: Operation,
    is_enemy_belligerent: Operation,
    is_observed: Operation,
    is_neutral: Operation,

    /// The true value of each action (the discounted estimate, anyway)
    action_values: Operation,

    // /// The predicted value of each action
    // action_values_hat: Operation,

    // // Operations
    // action_train_ops: Vec<Operation>,
}

impl DNN {
    fn tensors_for(&self, state: &Game) -> (Tensor<fX>,Tensor<fX>,Tensor<fX>,Tensor<fX>) {
        let x = state.features();

        //FIXME Splitting the input vector is something Keras should be doing but isn't quite ready to yet
        eprintln!("features_1d num outputs: {}", self.features_1d.num_outputs());
        eprintln!("features_1d num inputs: {}", self.features_1d.num_inputs());
        eprintln!("features_1d num control outputs: {}", self.features_1d.num_control_inputs());
        eprintln!("features_1d num control inputs: {}", self.features_1d.num_control_inputs());
        eprintln!("features_1d op type: {}", self.features_1d.op_type().unwrap());


        // eprintln!("features_1d num inputs: {}", self.features_1d.input_list_length(arg_name));

        // let sub_op = self.features_1d.input(1).0;
        // eprintln!("sub_op outputs: {}", sub_op.num_outputs());
        // eprintln!("sub_op inputs: {}", sub_op.num_inputs());

        let features_1d = Tensor::new(&[1_u64, 14_u64])
                                                        .with_values(&x[0..14])
                                                        .unwrap();
                                                        // .map_err(|err| LinearError{
                                                        //     kind: ErrorKind::Optimisation,
                                                        //     message: format!("Error training DNN: {}", err)
                                                        // })?;
        
        let mut base: usize = features_1d.dims()[0] as usize;

        let is_enemy_belligerent = Tensor::new(&[1_u64, 121_u64])
                                                        .with_values(&x[base..(base+121)])
                                                        .unwrap();
                                                        // .map_err(|err| LinearError{
                                                        //     kind: ErrorKind::Optimisation,
                                                        //     message: format!("Error training DNN: {}", err)
                                                        // })?;

        base += 121;

        let is_observed = Tensor::new(&[1_u64, 121_u64])
                                                        .with_values(&x[base..(base+121)])
                                                        .unwrap();
                                                        // .map_err(|err| LinearError{
                                                        //     kind: ErrorKind::Optimisation,
                                                        //     message: format!("Error training DNN: {}", err)
                                                        // })?;

        base += 121;


        let is_neutral = Tensor::new(&[1_u64, 121_u64])
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


        let regressand_idx = output_step.request_fetch(&self.action_values, *action as i32);

        self.session.run(&mut output_step)
                    .unwrap();
                    // .map_err(|err| LinearError{kind: ErrorKind::Evaluation, message: format!("Error evaluating DNN: {}", err)})
        
        // ?;

        let result = output_step.fetch(regressand_idx)
                    // .map_err(|err| LinearError{kind: ErrorKind::Evaluation, message: format!("Error evaluating DNN: {}", err)})
                    .unwrap().get(&[0, 0]);
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
        

        // let regressand_op = self.action_train_ops.get(*action).unwrap();

        // train_step.add_target(&regressand_op);

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

impl Loadable for DNN {
    fn load(path: &Path) -> Result<Self,String> {
        if !path.exists() {
            return Err(format!("Can't load DNN from path '{:?}' because it doesn't exist", path));
        }

        

        // Load the saved model exported by regression_savedmodel.py.
        let mut scope = Scope::new_root_scope();
        let (
            bundle,
            features_1d,
            is_enemy_belligerent,
            is_observed,
            is_neutral,
            action_values,
            // action_values_hat,
            // action_train_ops
        ) = {
            let mut graph = scope.graph_mut();
            let bundle = SavedModelBundle::load(
            // let session = Session::from_saved_model(
                &SessionOptions::new(),
                // &["train", "serve"],
                &[TAG],
                &mut graph,
                path,
            )
            .map_err(|err| {
                format!("Error loading saved model bundle from {}: {}", path.to_string_lossy(), err)
            })?;

            println!("===== Signatures =====");

            for (k,v) in bundle.meta_graph_def().signatures().iter() {
                println!("{:?} -> {:?}", k, v);
            }

            let features_1d = graph.operation_by_name_required("serving_default_1d_features")
            .map_err(|err| format!("Error getting 1d_features: {}", err))?;

            let is_enemy_belligerent = graph.operation_by_name_required("serving_default_is_enemy_belligerent")
            .map_err(|err| format!("Error getting is_enemy_belligerent: {}", err))?;

            let is_observed = graph.operation_by_name_required("serving_default_is_observed")
            .map_err(|err| format!("Error getting is_observed: {}", err))?;

            let is_neutral = graph.operation_by_name_required("serving_default_is_neutral")
            .map_err(|err| format!("Error getting is_neutral: {}", err))?;

            let action_values = graph.operation_by_name_required("StatefulPartitionedCall")
            .map_err(|err| format!("Error getting action_values (StatefulPartitionedCall): {}", err))?;

            eprintln!("Action values found: {}", action_values.num_outputs());

            // let action_values_hat = graph.operation_by_name_required("action_values_hat")
            // .map_err(|err| format!("Error getting action_values_hat: {}", err))?;

            // let action_train_ops = UmpireAction::possible_actions().iter().enumerate().map(|(i,action)| {
            //     graph.operation_by_name_required(format!("train_action_{}", i).as_str()).unwrap()
            // }).collect();

            (
                bundle,
                features_1d,
                is_enemy_belligerent,
                is_observed,
                is_neutral,
                action_values,
                // action_values_hat,
                // action_train_ops
            )
        };



        let session = bundle.session;
        Ok(Self {
            scope,
            session,
            features_1d,
            is_enemy_belligerent,
            is_observed,
            is_neutral,
            action_values,
            // action_values_hat,
            // action_train_ops,
        })
    }
}

impl Storable for DNN {
    fn store(mut self, path: &Path) -> Result<(),String> {
        let mut builder = SavedModelBuilder::new();
        builder.add_tag(TAG);

        let saver = builder.inject(&mut self.scope)
               .map_err(|status| {
                   format!("Error injecting scope into saved model builder, status {}", status)
               })?;

        let graph = self.scope.graph();

        saver.save(&self.session, &(*graph), path)
             .map_err(|err| format!("Error saving DNN: {}", err))
    }
}