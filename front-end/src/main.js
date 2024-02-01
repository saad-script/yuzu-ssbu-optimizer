import { createApp } from 'vue';
import { vuetify } from './plugins/vuetify';
import { unifiedApp } from './plugins/unified/unified-app';

import App from './app.vue';
import ProfileView from './modules/ProfileView/mod.vue'
import OptionCard from './modules/OptionCard/mod.vue'
import StatusIcon from './modules/StatusIcon/mod.vue'

function disableContextMenu() {
    document.addEventListener('contextmenu', e => {
        e.preventDefault();
        return false;
    }, { capture: true });
}
function disableSelection() {
    document.addEventListener('selectstart', e => {
        e.preventDefault();
        return false;
    }, { capture: true });
}

const app = createApp(App);
app.component('ProfileView', ProfileView);
app.component('OptionCard', OptionCard);
app.component('StatusIcon', StatusIcon);

app.use(vuetify);
app.use(unifiedApp);

disableContextMenu();
disableSelection();
app.mount('#app');
