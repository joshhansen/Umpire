import os

import numpy as np

import tensorflow as tf
from tensorflow.keras import Input, Model
from tensorflow.keras.layers import Concatenate, Conv2D, Dense, Dropout, Flatten, MaxPooling2D, Reshape
from tensorflow.keras.optimizers import Adam

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
        self.mse = tf.keras.losses.MeanSquaredError()
        self.optimizer = Adam()
        super().__init__(*args, **kwargs)
        

    @tf.function(input_signature=[
        tf.TensorSpec(shape=(1,14,), dtype=fX),#1d_features
        tf.TensorSpec(shape=(1,121,), dtype=fX),#is_enemy_belligerent
        tf.TensorSpec(shape=(1,121,), dtype=fX),#is_observed
        tf.TensorSpec(shape=(1,121,), dtype=fX),#is_neutral
        tf.TensorSpec(shape=(1,POSSIBLE_ACTIONS,), dtype=fX),# true_action_values
    ])
    def fit_action(self, _1d_features, is_enemy_belligerent, is_observed, is_neutral, true_action_values):
        inputs = [_1d_features, is_enemy_belligerent, is_observed, is_neutral, true_action_values]

        with tf.GradientTape() as tape:
            estimated_action_values = self.model(inputs)
            # mse = tf.keras.losses.MeanSquaredError()
            loss = self.mse(estimated_action_values, true_action_values)

        grads = tape.gradient(loss, model.trainable_variables)
        print(f"Gradients: {grads}")
        self.optimizer.apply_gradients(zip(grads, model.trainable_variables))

    #     # outputs_fun = tf.switch_case(action_idx, {
    #     #     0: 
    #     # })


    #     outputs = { name_tf(action_idx): true_action_value }
    #     self.model.fit(inputs, outputs, epochs=1)

#     # @tf.function(input_signature=[
#     #     tf.TensorSpec(shape=(14,), dtype=fX),#1d_features
#     #     tf.TensorSpec(shape=(121,), dtype=fX),#is_enemy_belligerent
#     #     tf.TensorSpec(shape=(121,), dtype=fX),#is_observed
#     #     tf.TensorSpec(shape=(121,), dtype=fX),#is_neutral
#     # ])
#     # @tf.function()
#     # def evaluate_all(self, _1d_features, is_enemy_belligerent, is_observed, is_neutral):
#     #     inputs = [_1d_features, is_enemy_belligerent, is_observed, is_neutral]
#     #     return self.model(inputs)

#     @tf.function()
#     def fit(self, _1d_features, is_enemy_belligerent, is_observed, is_neutral, action_values):
#         inputs = [_1d_features, is_enemy_belligerent, is_observed, is_neutral]
#         return self.model.fit(inputs, action_values)



if __name__=="__main__":

    # Disable GPU output which isn't needed just to build and serialize the graph
    os.environ['CUDA_VISIBLE_DEVICES'] = ""
    # tf.compat.v1.disable_eager_execution()
    tf.compat.v1.disable_v2_behavior()

    # tf.keras.backend.set_floatx('float16')
    # tf.keras.backend.set_floatx('float32')
    # tf.keras.backend.set_floatx('float64')
    
    input_1d = Input(shape=(14,), name='1d_features', dtype=fX)

    # input_true_action_values = [Input(shape=(1,), name=f"true_action_value{action_idx:02}") for action_idx in range(POSSIBLE_ACTIONS)]

    map_layer_names = ("is_enemy_belligerent", "is_observed", "is_neutral")
    map_inputs = list()
    for map_layer_name in map_layer_names:
        map_inputs.append(Input(shape=(121,), name=map_layer_name, dtype=fX))



    inputs_1d = [
        input_1d,
        # *input_true_action_values,
        *map_inputs
    ]



    # inputs_2d = list()
    # for i, input_1d in enumerate(map_inputs):
    #     inputs_2d.append(
    #         Reshape((11,11, 1), name="reshape_%s" % map_layer_names[i])(input_1d)
    #     )

    # layers = [inputs_2d]


    # for i, (filters, kernel_size) in enumerate([ (32, (3, 3)), (64, (3, 3)), (64, (3, 3)) ]):

    #     layer = list()

    #     # Iterate through the last layer's outputs which become this layer's inputs

    #     for j, input_2d in enumerate(layers[-1]):
    #         layer.append(
    #             Conv2D(filters, kernel_size=kernel_size, activation='relu', padding='valid',
    #                             name="conv2d_%s_%s" % (i, map_layer_names[j])
    #             )(input_2d)
    #         )
        
    #     layers.append(layer)

    # flattened = list()
    # for i, input_2d in enumerate(layers[-1]):
    #     flattened.append(
    #         Flatten(name="flatten_%s" % map_layer_names[i])(
    #             Dropout(0.25, name="dropout_%s" % map_layer_names[i])(
    #                 MaxPooling2D(pool_size=(2,2), name="max_pooling_%s" % map_layer_names[i])(input_2d)
    #             )
    #         )
    #     )

    # concatenated = tf.keras.layers.concatenate(
    #     [
    #         input_1d,
    #         # *flattened,
    #         *map_inputs
    #     ]
    # )

    concatenated = input_1d

    dense0 = Dense(64, activation='relu', name="dense0")(concatenated)
    # dropout0 = Dropout(0.1, name="dropout0")(dense0)
    # dense1 = Dense(32, activation='relu', name="dense1")(dropout0)
    # dropout1 = Dropout(0.1, name="dropout1")(dense1)



    # The estimate of the value
    # y_hat = Dense(1, activation='linear', name="y_hat")(dropout1)
    # action_values = Dense(POSSIBLE_ACTIONS, activation='linear', name='action_values')(dropout1)



    # final = dropout1
    final = dense0
    # final = input_1d
    
    action_value_estimates = list()
    true_action_values = Input([POSSIBLE_ACTIONS], dtype=fX, name='true_action_values')
    # true_action_values = list()
    losses = list()

    
    o = tf.keras.optimizers.Adam()
    for action_idx in range(POSSIBLE_ACTIONS):
        n = name(action_idx)
        action_value_estimate = Dense(1, activation='linear', name=n)(final)
        action_value_estimates.append(action_value_estimate)
        
        # true_action_value = Input([1], name=f"true_{n}")
        # true_action_values.append(true_action_value)

        true_action_value = true_action_values[0, action_idx]

        mse = tf.keras.losses.MeanSquaredError(name=f"mse_{n}")
        loss = mse(action_value_estimate, true_action_value)
        losses.append(loss)


    # print(input_1d)
    # print(map_inputs)

    
    model = Model(inputs=inputs_1d+[true_action_values], outputs=losses, name='umpire_regressor')
    model.summary()


    # optimizers = list()
    # o = tf.keras.optimizers.Adam()
    # for action_idx in range(POSSIBLE_ACTIONS):

    #     opt = o.minimize(lambda: losses[action_idx], model.trainable_variables)
    #     optimizers.append(opt)

    # model.some_extra_optimizers = optimizers

    # tf.keras.utils.plot_model(model)

    # trainable_weights = model.trainable_weights

    # # print(f"trainable_weights: {trainable_weights}")

    # optimizer = tf.keras.optimizers.Adam()
    # train_ops = list()
    # for action_idx in range(POSSIBLE_ACTIONS):
    #     action_value_estimate = action_value_estimates[action_idx]

    #     true_action_value = true_action_values[action_idx]

    #     # loss = lambda: tf.reduce_mean(tf.square(action_value_estimate - true_action_value))
    #     loss = lambda: tf.square(action_value_estimate - true_action_value)

    #     print(type(loss()))
        
    #     train_op = optimizer.minimize(loss, trainable_weights, name=f'train{action_idx}')
    #     train_ops.append(train_op)





    # model.compile(
    #     optimizer="sgd",
    #     loss="mse",
    #     metrics=["mse"],
    #     # target_tensors=[input_y],
    # )
    

    # model.summary()

    # model.optimizer.get_gradients(model.outputs[0], model.trainable_variables)

    
    # grads = list()
    # for action_idx in range(POSSIBLE_ACTIONS):
    #     grad = tf.keras.backend.gradients(model.outputs[action_idx], model.trainable_variables)
    #     # print(grad)
    #     print([x.name for x in grad if x is not None])
    #     grads.append(grad)

    # tf.keras.backend.clear_session()
    # tf.compat.v1.enable_eager_execution()

    # grads = list()
    # for action_idx in range(POSSIBLE_ACTIONS):
    #     mse = tf.keras.losses.MeanSquaredError(name=f"mse_{name(action_idx)}")

    #     with tf.GradientTape() as tape:
    #         tape.watch(model.trainable_variables)
    #         tape.watch(true_action_values)
    #         loss = mse(true_action_values[action_idx], action_value_estimates[action_idx])
    #         tape.watch(loss)

    #     grad = tape.gradient(loss, model.trainable_variables)
    #     print(grad)

    # model.extra_gradients = grads

    # model.fit(
    #     tf.constant([[0,1,2,3,4,5,6,7,8,9,10,11,12,13]]),
    #     tf.constant([[10]*19])
    # )

    # Save in SavedModel format
    # model.save('ai/umpire_regressor', save_format='tf', include_optimizer=True)
    # model.save('ai/umpire_regressor.h5', save_format='h5')

    trainable_ai = TrainableAI(model)
    

    _1d_features = tf.zeros((1,14,))
    is_enemy_belligerent = tf.zeros((1,121,))
    is_observed = tf.zeros((1,121,))
    is_neutral = tf.zeros((1,121,))
    true_action_values = tf.zeros((1,POSSIBLE_ACTIONS,))
    trainable_ai.fit_action(_1d_features, is_enemy_belligerent, is_observed, is_neutral, true_action_values)

    tf.saved_model.save(trainable_ai, 'ai/umpire_regressor')

    print(tf.autograph.to_code(trainable_ai.fit_action.python_function))

    # trainable_ai.save('ai/umpire_regressor', save_format='tf', include_optimizer=True)


    # #_1d_features, is_enemy_belligerent, is_observed, is_neutral, action_idx, true_action_value):
    # spec1d = tf.TensorSpec(shape=[1, 14], dtype=fX)
    # spec2d = tf.TensorSpec(shape=[1, 121], dtype=fX)

    # f_train_action = trainable_ai.fit.get_concrete_function(
    #     _1d_features = spec1d,
    #     is_enemy_belligerent = spec2d,
    #     is_observed = spec2d,
    #     is_neutral = spec2d,
    #     action_values = tf.TensorSpec(shape=[1, POSSIBLE_ACTIONS], dtype=fX, name="action_values")
    #     # action_idx = tf.TensorSpec(shape=[1], dtype=tf.uint32),
    #     # true_action_value = tf.TensorSpec(shape=[1], dtype=fX)
    # )

    # # f_evaluate_all = trainable_ai.evaluate_all.get_concrete_function(
    # #     _1d_features = spec1d,
    # #     is_enemy_belligerent = spec2d,
    # #     is_observed = spec2d,
    # #     is_neutral = spec2d,
    # # )

    # tf.saved_model.save(trainable_ai, 'ai/umpire_regressor',
    #     signatures = [f_train_action, f_evaluate_all]
    # )

    # # print(dir(model))


    # # model.add(Conv2D(32, kernel_size=(3, 3),
    # #                 activation='relu',
    # #                 input_shape=input_shape))
    # # model.add(Conv2D(64, (3, 3), activation='relu'))
    # # model.add(MaxPooling2D(pool_size=(2, 2)))
    # # model.add(Dropout(0.25))
    # # model.add(Flatten())
    # # model.add(Dense(128, activation='relu'))
    # # model.add(Dropout(0.5))
    # # model.add(Dense(num_classes, activation='softmax'))