import { defineCollection } from 'astro:content';
import { z } from 'zod';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema({ extend: z.object({ source: z.string().optional(), frameworkVersion: z.string().optional() }) }),
  }),
};
