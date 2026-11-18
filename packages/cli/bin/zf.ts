#!/usr/bin/env node
import { Command } from '../src/command.ts';
process.exitCode = await new Command().run(process.argv.slice(2));
