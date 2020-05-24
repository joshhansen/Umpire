import os

import tensorflow as tf
from tensorflow.keras import Input, Model
from tensorflow.keras.layers import Concatenate, Conv2D, Dense, Dropout, Flatten, MaxPooling2D, Reshape

POSSIBLE_ACTIONS = 19

fX = tf.float32

# The positions of these within the ultimate output vector are determined alphabetically, so we pad the index to
# to make things line up how we'd expect
def name(action_idx):
    return f"action_value_{action_idx:02}"

def name_tf(action_idx):
    return tf.strings.format("action_value_{:02)", action_idx)

class TrainableAI(tf.Module):
    def __init__(self, keras_model, *args, **kwargs):
        self.model = keras_model
        # print(type(self.model))
        super().__init__(*args, **kwargs)

    # @tf.function(input_signature=[
    #     tf.TensorSpec(shape=(14,), dtype=fX),#1d_features
    #     tf.TensorSpec(shape=(121,), dtype=fX),#is_enemy_belligerent
    #     tf.TensorSpec(shape=(121,), dtype=fX),#is_observed
    #     tf.TensorSpec(shape=(121,), dtype=fX),#is_neutral
    #     tf.TensorSpec(shape=(1,), dtype=tf.uint32),#action_idx
    #     tf.TensorSpec(shape=(1,), dtype=fX),#error
    # ])
    # @tf.function()
    # def fit_action(self, _1d_features, is_enemy_belligerent, is_observed, is_neutral, action_idx, true_action_value):
    #     inputs = [_1d_features, is_enemy_belligerent, is_observed, is_neutral]

    #     # outputs_fun = tf.switch_case(action_idx, {
    #     #     0: 
    #     # })


    #     outputs = { name_tf(action_idx): true_action_value }
    #     self.model.fit(inputs, outputs, epochs=1)

    # @tf.function(input_signature=[
    #     tf.TensorSpec(shape=(14,), dtype=fX),#1d_features
    #     tf.TensorSpec(shape=(121,), dtype=fX),#is_enemy_belligerent
    #     tf.TensorSpec(shape=(121,), dtype=fX),#is_observed
    #     tf.TensorSpec(shape=(121,), dtype=fX),#is_neutral
    # ])
    # @tf.function()
    # def evaluate_all(self, _1d_features, is_enemy_belligerent, is_observed, is_neutral):
    #     inputs = [_1d_features, is_enemy_belligerent, is_observed, is_neutral]
    #     return self.model(inputs)

    @tf.function()
    def fit(self, _1d_features, is_enemy_belligerent, is_observed, is_neutral, action_values):
        inputs = [_1d_features, is_enemy_belligerent, is_observed, is_neutral]
        return self.model.fit(inputs, action_values)



if __name__=="__main__":

    # Disable GPU output which isn't needed just to build and serialize the graph
    os.environ['CUDA_VISIBLE_DEVICES'] = ""

    # tf.keras.backend.set_floatx('float16')
    # tf.keras.backend.set_floatx('float32')
    # tf.keras.backend.set_floatx('float64')
    
    input_1d = Input(shape=(14,), name='1d_features', dtype=fX)

    
    action_value_estimates = list()
    for action_idx in range(POSSIBLE_ACTIONS):
        action_value_estimates.append(Dense(1, activation='linear', name=name(action_idx))(input_1d))


    model = Model(inputs=[input_1d], outputs=action_value_estimates, name='umpire_regressor')
    model.summary()

    # tf.keras.utils.plot_model(model)


    model.compile(
        optimizer="sgd",
        loss="mse",
        metrics=["mse"],
    )

    model.summary()

    model.fit(
        tf.constant([[0,1,2,3,4,5,6,7,8,9,10,11,12,13]]),
        tf.constant([[10]*19])
    )