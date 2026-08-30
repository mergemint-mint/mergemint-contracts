import React, { useEffect } from 'react';
import { BrowserRouter, Link, Route, Routes, useLocation } from 'react-router-dom';
import { WalletProvider, useWallet } from './lib/WalletContext';
import { WalletConnectButton } from './components/WalletConnectButton';
import { NetworkMismatchBanner } from './components/NetworkMismatchBanner';
import { BountyList } from './pages/BountyList';
import { BountyDetail } from './pages/BountyDetail';
import { CreateBounty } from './pages/CreateBounty';
import { ContributorProfile } from './pages/ContributorProfile';

function Nav() {
  const location = useLocation();
  const { clearError } = useWallet();

  // A prior connect() failure otherwise stays visible until the next
  // connect() attempt, even after navigating away (issue #508).
  useEffect(() => {
    clearError();
  }, [location.pathname, clearError]);

  return (
    <nav>
      <Link to="/">Bounties</Link>
      <Link to="/create">Create Bounty</Link>
      <WalletConnectButton />
    </nav>
  );
}

export default function App() {
  return (
    <WalletProvider>
      <BrowserRouter>
        <Nav />
        <NetworkMismatchBanner />
        <Routes>
          <Route path="/" element={<BountyList />} />
          <Route path="/bounties/:id" element={<BountyDetail />} />
          <Route path="/create" element={<CreateBounty />} />
          <Route path="/contributors/:address" element={<ContributorProfile />} />
        </Routes>
      </BrowserRouter>
    </WalletProvider>
  );
}
