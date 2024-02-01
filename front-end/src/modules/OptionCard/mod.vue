<template>
  <v-card class="mx-auto opt-card-main" :title="cardTitle" :subtitle="cardSubtitle" @click="toggle">
    <template v-slot:append>
      <StatusIcon :isCorrect="isOptimized" :correctMessage="'Optimized'" :incorrectMessage="'Not Optimized'"
        :location="'left'" />
    </template>
    <template v-slot:prepend>
      <v-icon :icon="cardDisplayIcon"></v-icon>
    </template>

    <v-checkbox-btn color="purple-lighten-2" class="opt-card-sub" v-model:model-value="isSelected"></v-checkbox-btn>
  </v-card>
</template>

<script>
export default {
  props: {
    cardTitle: String,
    cardSubtitle: String,
    cardDisplayIcon: String,
    isOptimized: Boolean,
  },
  data() {
    return {
      isSelected: true,
    };
  },
  methods: {
    toggle() {
      this.isSelected = !this.isSelected;
      this.$emit('toggled', this.isSelected);
    },
  }
};
</script>

<style scoped>
.opt-card-main {
  position: relative;
  padding: 20px;
  margin: 0px;
}

.opt-card-sub {
  position: absolute;
  top: 0;
  left: 0;
}
</style>
