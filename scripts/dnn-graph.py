import os

import tensorflow as tf
from tensorflow.keras import Input, Model
from tensorflow.keras.layers import Concatenate, Conv2D, Dense, Dropout, Flatten, MaxPooling2D, Reshape

if __name__=="__main__":

    # Disable GPU output which isn't needed just to build and serialize the graph
    os.environ['CUDA_VISIBLE_DEVICES'] = ""

    tf.keras.backend.set_floatx('float16')

    
    input_1d = Input(shape=(14,), name='1d_features')

    map_layer_names = ("is_enemy_belligerent", "is_observed", "is_neutral")
    map_inputs = list()
    for name in map_layer_names:
        map_inputs.append(Input(shape=(121,), name=name))

    # The true value
    input_y = Input(shape=(1,), name='y')


    inputs_1d = [ input_1d, *map_inputs, input_y ]



    inputs_2d = list()
    for i, input_1d in enumerate(map_inputs):
        inputs_2d.append(
            Reshape((11,11, 1), name="reshape_%s" % map_layer_names[i])(input_1d)
        )

    layers = [inputs_2d]


    for i, (filters, kernel_size) in enumerate([ (32, (3, 3)), (64, (3, 3)), (16, (5, 5)) ]):

        layer = list()

        # Iterate through the last layer's outputs which become this layer's inputs

        for j, input_2d in enumerate(layers[-1]):
            layer.append(
                Conv2D(filters, kernel_size=kernel_size, activation='relu', padding='valid',
                                name="conv2d_%s_%s" % (i, map_layer_names[j])
                )(input_2d)
            )
        
        layers.append(layer)

    flattened = list()
    for i, input_2d in enumerate(layers[-1]):
        flattened.append(
            Flatten(name="flatten_%s" % map_layer_names[i])(
                Dropout(0.25, name="dropout_%s" % map_layer_names[i])(
                    MaxPooling2D(pool_size=(2,2), name="max_pooling_%s" % map_layer_names[i])(input_2d)
                )
            )
        )

    concatenated = tf.keras.layers.concatenate(
        [
            input_1d,
            *flattened,
        ]
    )

    dense0 = Dense(64, activation='relu', name="dense0")(concatenated)
    dropout0 = Dropout(0.1, name="dropout0")(dense0)
    dense1 = Dense(32, activation='relu', name="dense1")(dropout0)
    dropout1 = Dropout(0.1, name="dropout1")(dense1)

    # The estimate of the value
    # y_hat = Dense(1, activation='linear', name="y_hat")(dropout1)
    action_values = Dense(19, activation='linear', name='action_values')(dropout1)

    # loss = tf.reduce_mean(tf.square(y_hat - y))
    # optimizer = tf.train.GradientDescentOptimizer(0.5)
    # train = optimizer.minimize(loss, name='train')



    model = Model(inputs=inputs_1d, outputs=[action_values], name='umpire_regressor')
    model.compile(
        optimizer="sgd",
        loss="mean_squared_error",
        metrics=["mse"],
        target_tensors=[input_y],
    )
    model.summary()

    # Save in SavedModel format
    model.save('ai/umpire_regressor', save_format='tf')

    # print(dir(model))


    # model.add(Conv2D(32, kernel_size=(3, 3),
    #                 activation='relu',
    #                 input_shape=input_shape))
    # model.add(Conv2D(64, (3, 3), activation='relu'))
    # model.add(MaxPooling2D(pool_size=(2, 2)))
    # model.add(Dropout(0.25))
    # model.add(Flatten())
    # model.add(Dense(128, activation='relu'))
    # model.add(Dropout(0.5))
    # model.add(Dense(num_classes, activation='softmax'))