import { mount } from 'svelte';
import App from './App.svelte';
import SignIn from './lib/SignIn.svelte';
import './app.css';
if (new URL(location.href).searchParams.has('signin')) mount(SignIn, { target: document.getElementById('app')! });
else mount(App, { target: document.getElementById('app')! });
