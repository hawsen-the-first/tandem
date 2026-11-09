import { render, screen } from '@testing-library/react';
import { App } from './App';

describe('App', () => {
  it('renders the placeholder heading in a main landmark', () => {
    render(<App />);
    expect(screen.getByRole('main')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Tandem' })).toBeInTheDocument();
  });
});
