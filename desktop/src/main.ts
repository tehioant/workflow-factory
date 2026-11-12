import { mount } from 'svelte';
import App from './App.svelte';
import SignIn from './lib/SignIn.svelte';
import './app.css';
mount(new URL(location.href).searchParams.has('signin') ? SignIn : App, { target: document.getElementById('app')! });
